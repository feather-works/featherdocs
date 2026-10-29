//! A aba Desenhar: canetas, marca-texto, borrachas, laço e tinta para forma sobre as páginas, com
//! os controles e a camada comuns da suíte (`feather_ui::tinta`, os mesmos do FeatherSheet) e o
//! motor do `feather_draw::tinta`. A tinta mora em `Extras::tinta`, uma por página, em pontos da
//! página. O `mudar_extras` do motor não entra no desfazer, então cada traço ou apagada guarda aqui
//! o texto de antes: Ctrl+Z logo depois desfaz a tinta (`desfazer_tinta`).
// shortcut: a tinta fica presa à página (não ao parágrafo) e não vai para o .docx; ancorar ao bloco e
// gravar como `w:ink`/DrawingML quando alguém pedir.
use crate::app::{App, Msg};
use feather_draw::tinta::{Ferramenta as FerramentaMotor, PontoTinta, Tinta, Traco};
use feather_draw::{Cor, Ponto};
use feather_ui::tinta::{self as ui, Camada, EventoCamada, Ferramenta, TracoTela, Transformacao};
use pelicano::{Element, Length};
use std::cell::RefCell;

/// Teto de traços por página (o texto `FTINTA` cresce linha a linha).
const MAX_TRACOS: usize = 2000;

#[derive(Default)]
pub struct EstadoTinta {
    /// Ferramenta, canetas, modo e o menu de cor e espessura.
    pub ui: ui::Estado,
    /// Onde o menu de cor e espessura abre (abaixo do botão clicado, na área abaixo da barra).
    pub menu_pos: (f32, f32),
    /// Apagando: a página, a tinta já apagada, se algo mudou e o último ponto (vira um Ctrl+Z ao soltar).
    rascunho: Option<(usize, Tinta, bool, Option<Ponto>)>,
    /// Traços escolhidos com o laço (Delete apaga).
    pub selecao: Option<(usize, Vec<u64>)>,
    /// O texto de antes de cada mudança de tinta (página, texto) e a versão do documento depois da
    /// última: editar outra coisa no meio esvazia a pilha (o Ctrl+Z passa a ser do texto).
    desfazer: Vec<(usize, String)>,
    versao_desfazer: u64,
    /// O texto lido da última vez e a tinta dele, para não reler a cada quadro.
    cache: RefCell<(Vec<String>, Vec<Tinta>)>,
}

impl EstadoTinta {
    /// A tinta de cada página (lida do texto guardado; vazio = sem tinta).
    fn paginas(&self, textos: &[String]) -> Vec<Tinta> {
        let mut c = self.cache.borrow_mut();
        if c.0 != textos {
            c.1 = textos.iter().map(|t| if t.is_empty() { Tinta::nova() } else { Tinta::de_texto(t).unwrap_or_default() }).collect();
            c.0 = textos.to_vec();
        }
        c.1.clone()
    }
}

fn cor_motor(c: [u8; 4]) -> Cor {
    Cor::rgba(c[0], c[1], c[2], c[3])
}

impl App {
    fn tinta_da_pagina(&self, pagina: usize) -> Tinta {
        if let Some((p, t, ..)) = &self.tinta.rascunho {
            if *p == pagina {
                return t.clone();
            }
        }
        self.tinta.paginas(&self.sessao.documento().extras.tinta).into_iter().nth(pagina).unwrap_or_default()
    }

    /// Grava a tinta da página: uma etapa de desfazer.
    fn gravar_tinta(&mut self, pagina: usize, tinta: Tinta, status: &str) {
        let texto = if tinta.is_empty() { String::new() } else { tinta.para_texto() };
        if self.sessao.versao() != self.tinta.versao_desfazer {
            self.tinta.desfazer.clear();
        }
        let antes = self.sessao.documento().extras.tinta.get(pagina).cloned().unwrap_or_default();
        self.tinta.desfazer.push((pagina, antes));
        self.escrever_tinta(pagina, texto);
        self.status = crate::localidade::tr(status).into();
    }

    /// Ctrl+Z logo depois de desenhar ou apagar: volta a tinta de antes. `false` = não era tinta.
    pub(crate) fn desfazer_tinta(&mut self) -> bool {
        if self.sessao.versao() != self.tinta.versao_desfazer {
            return false;
        }
        let Some((pagina, texto)) = self.tinta.desfazer.pop() else { return false };
        self.tinta.selecao = None;
        self.escrever_tinta(pagina, texto);
        self.status = crate::localidade::tr("Tinta desfeita.").into();
        true
    }

    fn escrever_tinta(&mut self, pagina: usize, texto: String) {
        let _ = self.sessao.mudar_extras(|x| {
            if x.tinta.len() <= pagina {
                x.tinta.resize(pagina + 1, String::new());
            }
            x.tinta[pagina] = texto;
            // Sem páginas vazias sobrando no fim.
            while x.tinta.last().is_some_and(String::is_empty) {
                x.tinta.pop();
            }
        });
        self.tinta.versao_desfazer = self.sessao.versao();
    }

    /// A página com a camada de tinta por cima (só se há tinta nela ou o modo desenho está ligado).
    pub(crate) fn pagina_com_tinta<'a>(&'a self, pagina: usize, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let estado = &self.tinta.ui;
        let tinta = self.tinta_da_pagina(pagina);
        if !estado.ativo && tinta.is_empty() {
            return base;
        }
        let tracos = tinta.tracos().iter().map(|t| {
            let alfa = f32::from(t.cor.a) / 255. * t.opacidade;
            TracoTela {
                id: t.id,
                pontos: t.pontos.iter().map(|p| pelicano::Point::new(p.x, p.y)).collect(),
                cor: pelicano::Color::rgb8(t.cor.r, t.cor.g, t.cor.b).alpha(alfa.clamp(0., 1.)),
                espessura: t.espessura,
                reto: t.reto,
                marca_texto: t.ferramenta == FerramentaMotor::MarcaTexto,
                contorno: Vec::new(),
            }
        }).collect();
        let transformacao = Transformacao { escala: self.config.zoom, dx: 0., dy: 0. };
        // A camada devolve o evento cru; o `map` diz de que página ele veio.
        let mut camada = Camada::nova(tracos, transformacao, |e| e);
        camada.ferramenta = estado.ferramenta_ativa();
        camada.pena = estado.pena().map(|p| {
            let opacidade = if estado.ferramenta == Ferramenta::MarcaTexto { FerramentaMotor::MarcaTexto.opacidade_padrao() } else { 1. };
            let [r, g, b, a] = p.cor;
            (pelicano::Color::rgb8(r, g, b).alpha(f32::from(a) / 255. * opacidade), p.espessura)
        });
        if let Some((p, ids)) = &self.tinta.selecao {
            if *p == pagina {
                camada.selecionados = ids.clone();
            }
        }
        let camada = ui::camada(camada).map(move |e| Msg::Tinta(pagina, e));
        pelicano::stack![base, camada].width(Length::Shrink).height(Length::Shrink).into()
    }

    /// Os botões da aba Desenhar.
    pub(crate) fn evento_desenhar(&mut self, evento: ui::Evento) {
        let menu_antes = self.tinta.ui.menu;
        let parar = evento == ui::Evento::Parar;
        if matches!(evento, ui::Evento::Escolher(f) if f != Ferramenta::Laco) {
            self.tinta.selecao = None;
        }
        self.tinta.ui.atualizar(evento);
        if self.tinta.ui.menu.is_some() && self.tinta.ui.menu != menu_antes {
            let p = self.ponteiro_da_faixa();
            self.tinta.menu_pos = ((p.x - 30.).max(8.), p.y + 26.);
        }
        use crate::localidade::{tr, trf};
        if parar {
            self.tinta.selecao = None;
            self.tinta.rascunho = None;
            self.status = tr("Modo de edição: clique no texto normalmente.").into();
        } else if self.tinta.ui.ativo && self.tinta.ui.menu.is_none() {
            let f = self.tinta.ui.ferramenta;
            self.status = match f {
                f if f.desenha() => trf("{}: desenhe sobre a página (Esc para parar).", &[&tr(f.rotulo())]),
                Ferramenta::Laco => tr("Laço: circule os traços para selecionar (Delete apaga).").into(),
                f => trf("{}: passe sobre os traços (Esc para parar).", &[&tr(f.rotulo())]),
            };
        }
    }

    /// O que a camada da página `pagina` capturou (em pontos da página).
    pub(crate) fn evento_tinta(&mut self, pagina: usize, evento: EventoCamada) {
        match evento {
            EventoCamada::TracoTerminado { ferramenta, pontos } => {
                let Some(pena) = self.tinta.ui.pena_de(ferramenta) else { return };
                let motor = if ferramenta == Ferramenta::MarcaTexto { FerramentaMotor::MarcaTexto } else { FerramentaMotor::Caneta };
                let pontos = pontos.iter().map(|p| PontoTinta::novo(p.x, p.y).com_tempo(p.tempo_ms)).collect();
                let mut traco = Traco::novo(pontos, cor_motor(pena.cor), pena.espessura, motor);
                traco.preparar(0.6 / self.config.zoom);
                let mut status = "Traço desenhado (Ctrl+Z desfaz).";
                if self.tinta.ui.tinta_para_forma {
                    if let Some(forma) = traco.reconhecer_forma() {
                        traco = traco.com_forma(&forma);
                        status = "Tinta convertida em forma (Ctrl+Z desfaz).";
                    }
                }
                let mut tinta = self.tinta_da_pagina(pagina);
                if tinta.len() >= MAX_TRACOS {
                    self.status = crate::localidade::trf("Limite de {} traços por página.", &[&MAX_TRACOS]);
                    return;
                }
                if tinta.adicionar(traco).is_some() {
                    self.gravar_tinta(pagina, tinta, status);
                }
            }
            EventoCamada::Apagar { ferramenta, ponto, raio } => {
                let (_, mut tinta, mut mudou, ultimo) = self.tinta.rascunho.take()
                    .filter(|(p, ..)| *p == pagina)
                    .unwrap_or_else(|| (pagina, self.tinta_da_pagina(pagina), false, None));
                let p = Ponto::novo(ponto.x, ponto.y);
                let caminho: Vec<Ponto> = ultimo.into_iter().chain([p]).collect();
                mudou |= match ferramenta {
                    Ferramenta::BorrachaPonto => tinta.apagar_pontos_no_caminho(&caminho, raio),
                    _ => !tinta.apagar_tracos_no_caminho(&caminho, raio).is_empty(),
                };
                self.tinta.rascunho = Some((pagina, tinta, mudou, Some(p)));
            }
            EventoCamada::ApagarFim => {
                if let Some((p, tinta, true, _)) = self.tinta.rascunho.take() {
                    self.tinta.selecao = None;
                    self.gravar_tinta(p, tinta, "Tinta apagada (Ctrl+Z desfaz).");
                }
            }
            EventoCamada::Laco(poligono) => {
                let laco: Vec<Ponto> = poligono.iter().map(|p| Ponto::novo(p.x, p.y)).collect();
                let ids = self.tinta_da_pagina(pagina).selecionar_laco(&laco);
                use crate::localidade::{tr, trf};
                self.status = match ids.len() {
                    0 => tr("Nenhum traço dentro do laço.").into(),
                    1 => tr("1 traço selecionado (Delete apaga).").into(),
                    n => trf("{} traços selecionados (Delete apaga).", &[&n]),
                };
                self.tinta.selecao = (!ids.is_empty()).then_some((pagina, ids));
            }
        }
    }

    /// Delete com traços escolhidos pelo laço: apaga (um Ctrl+Z). `false` = não havia o que apagar.
    pub(crate) fn apagar_selecao_tinta(&mut self) -> bool {
        let Some((pagina, ids)) = self.tinta.selecao.take() else { return false };
        let mut tinta = self.tinta_da_pagina(pagina);
        for id in ids {
            tinta.remover(id);
        }
        self.gravar_tinta(pagina, tinta, "Tinta apagada (Ctrl+Z desfaz).");
        true
    }

    /// Modo de captura: um rabisco de cada caneta, marca-texto e uma forma reconhecida.
    pub(crate) fn preparar_captura_tinta(&mut self, menu: bool) {
        use feather_ui::tinta::PontoCaptura;
        let riscar = |app: &mut App, f: Ferramenta, pts: Vec<(f32, f32)>| {
            app.evento_desenhar(ui::Evento::Escolher(f));
            let pontos = pts.into_iter().enumerate().map(|(i, (x, y))| PontoCaptura::novo(x, y, i as u32 * 8)).collect();
            app.evento_tinta(0, EventoCamada::TracoTerminado { ferramenta: f, pontos });
        };
        riscar(self, Ferramenta::MarcaTexto, vec![(70., 76.), (260., 78.)]);
        riscar(self, Ferramenta::Caneta(0), (0..40).map(|i| (300. + i as f32 * 4., 200. + (i as f32 * 0.4).sin() * 18.)).collect());
        self.evento_desenhar(ui::Evento::AlternarFormas);
        riscar(self, Ferramenta::Caneta(1), (0..48).map(|i| {
            let a = i as f32 / 47. * std::f32::consts::TAU;
            (160. + a.cos() * 60., 300. + a.sin() * 35.)
        }).collect());
        self.evento_desenhar(ui::Evento::AlternarFormas);
        if menu {
            self.tinta.menu_pos = (80., 120.);
            self.tinta.ui.atualizar(ui::Evento::AlternarMenu(Ferramenta::Caneta(1)));
        }
    }

    /// O menu flutuante de cor e espessura das canetas (sobre o corpo, abaixo da barra do título).
    pub(crate) fn sobrepor_menu_tinta<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if ui::menu_pena(&self.tinta.ui).is_none() {
            // Fechado, a mesma pilha (ver `App::view`): o caminho da tela na árvore não muda ao abrir.
            return pelicano::stack![base].width(pelicano::Length::FILL).height(pelicano::Length::FILL).into();
        }
        let (x, y) = self.tinta.menu_pos;
        ui::sobrepor_menu(base, &self.tinta.ui, x, y, self.tamanho_janela(), Msg::Desenhar)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feather_ui::tinta::PontoCaptura;

    fn rabisco(app: &mut App, f: Ferramenta, pagina: usize, pts: &[(f32, f32)]) {
        app.evento_desenhar(ui::Evento::Escolher(f));
        let pontos = pts.iter().map(|&(x, y)| PontoCaptura::novo(x, y, 0)).collect();
        app.evento_tinta(pagina, EventoCamada::TracoTerminado { ferramenta: f, pontos });
    }

    #[test]
    fn desenhar_apagar_e_desfazer_na_pagina() {
        let mut app = App::default();
        rabisco(&mut app, Ferramenta::Caneta(0), 0, &[(50., 50.), (120., 60.), (200., 90.)]);
        rabisco(&mut app, Ferramenta::MarcaTexto, 0, &[(50., 200.), (300., 200.)]);
        assert_eq!(app.tinta_da_pagina(0).len(), 2);
        // Borracha de traço passando pelo primeiro: some inteiro, num Ctrl+Z só.
        app.evento_desenhar(ui::Evento::Escolher(Ferramenta::BorrachaTraco));
        app.evento_tinta(0, EventoCamada::Apagar { ferramenta: Ferramenta::BorrachaTraco, ponto: pelicano::Point::new(120., 60.), raio: 4. });
        app.evento_tinta(0, EventoCamada::ApagarFim);
        assert_eq!(app.tinta_da_pagina(0).len(), 1);
        let _ = app.update(Msg::Comando(crate::app::Comando::Desfazer));
        assert_eq!(app.tinta_da_pagina(0).len(), 2);
        // Laço em volta do marca-texto e Delete.
        app.evento_desenhar(ui::Evento::Escolher(Ferramenta::Laco));
        app.evento_tinta(0, EventoCamada::Laco(vec![
            pelicano::Point::new(30., 180.), pelicano::Point::new(330., 180.), pelicano::Point::new(330., 230.), pelicano::Point::new(30., 230.)]));
        assert!(app.apagar_selecao_tinta());
        assert_eq!(app.tinta_da_pagina(0).len(), 1);
        // A tinta vai no arquivo.
        let bytes = featherdocs_io::fdoz::gravar(app.sessao.documento()).expect("grava");
        let lido = featherdocs_io::fdoz::ler(&bytes).expect("lê");
        assert_eq!(lido.extras.tinta, app.sessao.documento().extras.tinta);
        assert!(!lido.extras.tinta.is_empty());
    }

    #[test]
    fn tinta_para_forma_endireita_a_linha() {
        let mut app = App::default();
        app.evento_desenhar(ui::Evento::AlternarFormas);
        let pts: Vec<(f32, f32)> = (0..30).map(|i| (40. + i as f32 * 8., 100. + if i % 2 == 0 { 0.8 } else { -0.8 })).collect();
        rabisco(&mut app, Ferramenta::Caneta(1), 1, &pts);
        let t = app.tinta_da_pagina(1);
        assert_eq!(t.len(), 1);
        assert!(t.tracos()[0].reto, "a linha tremida vira reta");
    }
}
