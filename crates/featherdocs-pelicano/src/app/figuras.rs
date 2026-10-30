//! Figuras na página: selecionar, mover e redimensionar pelas alças, apagar, e colar um print.
//! As do papel timbrado (as de toda página) só se pegam editando o cabeçalho ou o rodapé, como no Word.
use super::{App, Msg};
use crate::compat::keyboard::{key::Named, Key};
use pelicano::{Point, Rect, Size, Task};

/// Uma figura presa à página (`Extras::decoracoes`) ou uma imagem do texto (o bloco).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Alvo {
    Decoracao(usize),
    Imagem(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Selecao {
    pub alvo: Alvo,
    pub pagina: usize,
    pub arrasto: Option<Arrasto>,
}

/// O arrasto: a alça pega (0 a 7, de cima à esquerda no sentido do relógio; `None` = mover a
/// figura inteira), o retângulo de antes, onde o mouse apertou e onde está.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Arrasto {
    pub alca: Option<usize>,
    pub ret: [f32; 4],
    pub de: (f32, f32),
    pub agora: (usize, f32, f32),
}

/// As oito alças: cantos e meios dos lados.
pub(crate) fn alcas(r: [f32; 4]) -> [(f32, f32); 8] {
    let [x, y, w, h] = r;
    [(x, y), (x + w / 2.0, y), (x + w, y), (x + w, y + h / 2.0), (x + w, y + h), (x + w / 2.0, y + h), (x, y + h), (x, y + h / 2.0)]
}

fn alca_no_ponto(r: [f32; 4], x: f32, y: f32, tolerancia: f32) -> Option<usize> {
    alcas(r).iter().position(|(ax, ay)| (ax - x).abs() <= tolerancia && (ay - y).abs() <= tolerancia)
}

/// O retângulo depois do arrasto. Os cantos mantêm a proporção (como no Word); os lados esticam.
pub(crate) fn arrastado(a: &Arrasto) -> [f32; 4] {
    let [x, y, w, h] = a.ret;
    let (dx, dy) = (a.agora.1 - a.de.0, a.agora.2 - a.de.1);
    let Some(alca) = a.alca else { return [x + dx, y + dy, w, h] };
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w, y + h);
    match alca {
        0 | 6 | 7 => x0 += dx,
        2..=4 => x1 += dx,
        _ => {}
    }
    match alca {
        0..=2 => y0 += dy,
        4..=6 => y1 += dy,
        _ => {}
    }
    let (mut nw, mut nh) = ((x1 - x0).max(8.0), (y1 - y0).max(8.0));
    if matches!(alca, 0 | 2 | 4 | 6) && w > 0.0 && h > 0.0 {
        // O eixo que o mouse mais mexeu manda (para crescer e para diminuir).
        let (ex, ey) = (nw / w, nh / h);
        let escala = if (ex - 1.0).abs() >= (ey - 1.0).abs() { ex } else { ey };
        (nw, nh) = ((w * escala).max(8.0), (h * escala).max(8.0));
    }
    // O lado oposto à alça fica parado.
    let nx = if matches!(alca, 0 | 6 | 7) { x + w - nw } else { x };
    let ny = if matches!(alca, 0..=2) { y + h - nh } else { y };
    [nx, ny, nw, nh]
}

impl App {
    /// Onde a figura selecionada está agora (na página dela).
    pub(crate) fn ret_da_figura(&self, s: &Selecao) -> Option<[f32; 4]> {
        match s.alvo {
            Alvo::Decoracao(k) => self.sessao.figura_retangulo(k),
            Alvo::Imagem(id) => self.sessao.imagens_na_pagina(s.pagina).into_iter().find(|(i, _)| *i == id).map(|(_, r)| r)
                .or_else(|| self.sessao.imagens_soltas_na_pagina(s.pagina).into_iter().find(|(i, ..)| *i == id).map(|(_, r, _)| [r.x, r.y, r.w, r.h])),
        }
    }

    /// As alças a desenhar na página: a figura selecionada ou a imagem solta selecionada.
    pub(crate) fn alcas_na_pagina(&self, pagina: usize) -> Option<Rect> {
        let r = match (&self.figura, &self.solta) {
            (Some(s), _) if s.pagina == pagina => self.ret_da_figura(s)?,
            (None, Some(solta)) => self.sessao.imagens_soltas_na_pagina(pagina).into_iter().find(|(i, ..)| *i == solta.id).map(|(_, r, _)| [r.x, r.y, r.w, r.h])?,
            _ => return None,
        };
        Some(Rect::new(Point::new(r[0], r[1]), Size::new(r[2], r[3])))
    }

    /// A sombra da figura sendo arrastada ou redimensionada.
    pub(crate) fn fantasma_da_figura(&self, pagina: usize) -> Option<Rect> {
        let a = self.figura.as_ref()?.arrasto.as_ref().filter(|a| a.agora.0 == pagina && (a.agora.1, a.agora.2) != a.de)?;
        let r = arrastado(a);
        Some(Rect::new(Point::new(r[0], r[1]), Size::new(r[2], r[3])))
    }

    /// O clique simples: na alça da selecionada, numa figura presa à página (as do timbre só editando o
    /// cabeçalho) ou numa imagem do texto. `true` = o clique ficou com a figura.
    pub(crate) fn clicar_figura(&mut self, p: usize, x: f32, y: f32) -> bool {
        let tolerancia = 6.0 / self.config.zoom.max(0.1);
        let selecionada = self.figura.map(|s| (s, self.ret_da_figura(&s))).or_else(|| {
            let solta = self.solta.as_ref()?;
            let s = Selecao { alvo: Alvo::Imagem(solta.id), pagina: p, arrasto: None };
            Some((s, self.ret_da_figura(&s)))
        });
        if let Some((s, Some(r))) = selecionada.filter(|(s, _)| s.pagina == p) {
            if let Some(alca) = alca_no_ponto(r, x, y, tolerancia) {
                self.solta = None;
                self.figura = Some(Selecao { arrasto: Some(Arrasto { alca: Some(alca), ret: r, de: (x, y), agora: (p, x, y) }), ..s });
                return true;
            }
        }
        self.figura = None;
        if self.sessao.documento().extras.protegido {
            return false;
        }
        let timbre = self.sessao.faixa_em_edicao().is_some();
        if let Some(k) = self.sessao.figura_no_ponto(p, x, y, timbre) {
            let r = self.sessao.figura_retangulo(k).unwrap_or_default();
            self.solta = None;
            self.figura = Some(Selecao { alvo: Alvo::Decoracao(k), pagina: p, arrasto: Some(Arrasto { alca: None, ret: r, de: (x, y), agora: (p, x, y) }) });
            return true;
        }
        // A imagem do texto fica selecionada (com as alças), e o clique segue: o cursor vai para ela.
        if let Some((id, _)) = self.sessao.imagens_na_pagina(p).into_iter().find(|(_, r)| (r[0]..=r[0] + r[2]).contains(&x) && (r[1]..=r[1] + r[3]).contains(&y)) {
            self.figura = Some(Selecao { alvo: Alvo::Imagem(id), pagina: p, arrasto: None });
        }
        false
    }

    /// O botão solto: aplica o arrasto (mover ou redimensionar), num Ctrl+Z.
    pub(crate) fn soltar_figura(&mut self) {
        let Some(s) = self.figura.as_mut() else { return };
        let Some(a) = s.arrasto.take() else { return };
        let alvo = s.alvo;
        if (a.agora.1 - a.de.0).hypot(a.agora.2 - a.de.1) < 2.0 {
            return;
        }
        let r = arrastado(&a);
        let feito = match alvo {
            Alvo::Decoracao(k) => self.sessao.figura_ajustar(k, r).map(|_| ()),
            Alvo::Imagem(id) if a.alca.is_some() => self.sessao.imagem_redimensionar(id, r[2], r[3]).map(|_| ()),
            Alvo::Imagem(_) => Ok(()),
        };
        if let Err(e) = feito {
            self.status = e;
        }
        self.atualizar();
    }

    /// Delete apaga a figura presa à página; Esc tira a seleção. `true` = a tecla ficou com a figura.
    pub(crate) fn tecla_na_figura(&mut self, k: &Key) -> bool {
        let Some(s) = self.figura else { return false };
        match (k, s.alvo) {
            (Key::Named(Named::Escape), _) => self.figura = None,
            (Key::Named(Named::Delete | Named::Backspace), Alvo::Decoracao(i)) => {
                self.figura = None;
                if let Err(e) = self.sessao.figura_apagar(i) {
                    self.status = e;
                }
                self.atualizar();
            }
            // Outra tecla com uma figura presa à página selecionada: a seleção sai e a tecla segue.
            (_, Alvo::Decoracao(_)) => {
                self.figura = None;
                return false;
            }
            _ => return false,
        }
        true
    }

    /// Ctrl+V: um print (imagem na área de transferência) entra como imagem; senão, cola o texto.
    pub(crate) fn colar(&mut self) -> Task<Msg> {
        Task::perform(
            async { tokio::task::spawn_blocking(ler_print).await.ok().flatten() },
            Msg::ColadoImagem,
        )
    }

    pub(crate) fn colado_imagem(&mut self, r: Option<Result<(String, f32, f32, Option<String>), String>>) -> Task<Msg> {
        match r {
            None => return Task::read_clipboard(Msg::Colado),
            Some(Err(e)) => self.status = e,
            // Editando o cabeçalho, o print vai para o papel timbrado; senão, para o texto.
            Some(Ok((origem, w, h, aviso))) if self.sessao.faixa_em_edicao().is_some() => match self.sessao.timbre_inserir(&origem, w, h) {
                Ok(k) => {
                    let pagina = self.sessao.faixa_em_edicao().map_or(0, |(p, _)| p);
                    self.figura = Some(Selecao { alvo: Alvo::Decoracao(k), pagina, arrasto: None });
                    self.status = aviso.unwrap_or_else(|| crate::localidade::tr("Imagem colada no papel timbrado: arraste e use as alças").into());
                }
                Err(e) => self.status = e,
            },
            Some(Ok((origem, w, h, aviso))) => match self.sessao.inserir_imagem(&origem, w, h) {
                Ok(_) => self.status = aviso.unwrap_or_default(),
                Err(e) => self.status = e,
            },
        }
        self.atualizar();
        Task::none()
    }
}

/// O print da área de transferência, pronto para inserir; `None` se ela não tem imagem.
fn ler_print() -> Option<Result<(String, f32, f32, Option<String>), String>> {
    let dados = arboard::Clipboard::new().ok()?.get_image().ok()?;
    let (w, h) = (u32::try_from(dados.width).ok()?, u32::try_from(dados.height).ok()?);
    let rgba = image::RgbaImage::from_raw(w, h, dados.bytes.into_owned())?;
    let mut png = Vec::new();
    if let Err(e) = image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png) {
        return Some(Err(e.to_string()));
    }
    Some(crate::imagem_compartilhada::preparar_insercao(&png))
}


#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn canto_mantem_a_proporcao_e_lado_estica() {
        let a = |alca, dx: f32, dy: f32| arrastado(&Arrasto { alca, ret: [100.0, 100.0, 200.0, 100.0], de: (0.0, 0.0), agora: (0, dx, dy) });
        assert_eq!(a(None, 10.0, 5.0), [110.0, 105.0, 200.0, 100.0]);
        assert_eq!(a(Some(4), 200.0, 0.0), [100.0, 100.0, 400.0, 200.0], "canto de baixo à direita dobra");
        assert_eq!(a(Some(3), 50.0, 30.0), [100.0, 100.0, 250.0, 100.0], "o lado direito só estica a largura");
        assert_eq!(a(Some(0), 100.0, 0.0), [200.0, 150.0, 100.0, 50.0], "o canto de cima à esquerda deixa o oposto parado");
    }
}
