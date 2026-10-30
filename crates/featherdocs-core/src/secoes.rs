//! As seções do documento, como no Word: cada quebra de seção fecha a seção de antes e guarda o
//! papel, o cabeçalho e o rodapé dela (`Bloco::secao`; `None` = os do documento); a última seção
//! é a do documento. A página pega o papel e as faixas da seção do primeiro bloco dela.
use crate::modelo::{ConfigPagina, Conteudo, Faixas, Secao};
use crate::sessao::{Mudanca, Sessao};

/// O que a aba Layout mostra da seção do cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct InfoSecao {
    /// A seção (0, 1…) e quantas há.
    pub numero: usize,
    pub total: usize,
    /// As páginas (0, 1…) em que ela aparece.
    pub paginas: (usize, usize),
    pub paisagem: bool,
    /// O número da primeira página dela, se ela recomeça a contagem (0 = continua).
    pub reinicia_em: u32,
    /// O cabeçalho e o rodapé dela são os mesmos da seção de antes.
    pub igual_a_anterior: bool,
}

impl Sessao {
    /// Os índices das quebras de seção, na ordem: a seção `k` termina na quebra `k` (a última, no fim).
    fn quebras_de_secao(&self) -> Vec<usize> {
        self.doc.blocos.iter().enumerate().filter(|(_, b)| matches!(b.conteudo, Conteudo::QuebraDeSecao { .. })).map(|(i, _)| i).collect()
    }

    /// A seção do bloco `i`: quantas quebras de seção vêm antes dele (a quebra é da seção que fecha).
    pub(crate) fn secao_do_bloco(&self, i: usize) -> usize {
        self.doc.blocos[..i].iter().filter(|b| matches!(b.conteudo, Conteudo::QuebraDeSecao { .. })).count()
    }

    pub fn total_de_secoes(&self) -> usize {
        self.quebras_de_secao().len() + 1
    }

    /// O que a seção `k` guarda (`None` = a última, ou uma que segue o documento).
    fn dados_da_secao(&self, k: usize) -> Option<&Secao> {
        self.quebras_de_secao().get(k).and_then(|&i| self.doc.blocos[i].secao.as_deref())
    }

    /// O papel e as margens da seção `k`.
    pub fn cfg_da_secao(&self, k: usize) -> ConfigPagina {
        self.dados_da_secao(k).map_or(self.doc.pagina, |s| s.pagina)
    }

    /// O cabeçalho e o rodapé da seção `k`.
    pub(crate) fn faixas_da_secao(&self, k: usize) -> (&Faixas, &Faixas) {
        match self.dados_da_secao(k) {
            Some(s) => (&s.cabecalho, &s.rodape),
            None => (&self.doc.extras.cabecalho, &self.doc.extras.rodape),
        }
    }

    /// O papel da página `p` (o do documento se ela não existe).
    pub fn cfg_da_pagina(&self, p: usize) -> ConfigPagina {
        self.paginas.get(p).map_or(self.doc.pagina, |p| p.cfg)
    }

    /// O papel do bloco `i` (o da seção dele).
    pub(crate) fn cfg_do_bloco(&self, i: usize) -> ConfigPagina {
        self.cfg_da_secao(self.secao_do_bloco(i))
    }

    /// O número que a página `p` (0, 1…) mostra: a primeira seção começa em `Extras::pagina_inicial`;
    /// as outras continuam a contagem ou recomeçam no número que a quebra de antes delas guarda.
    pub fn numero_da_pagina(&self, p: usize) -> usize {
        let quebras = self.quebras_de_secao();
        let mut numero = self.doc.extras.pagina_inicial as usize;
        for (k, pag) in self.paginas.iter().enumerate().take(p + 1) {
            let comeca_secao = k > 0 && pag.secao != self.paginas[k - 1].secao;
            let reinicia = comeca_secao.then(|| quebras.get(pag.secao.wrapping_sub(1)).and_then(|&i| self.doc.blocos[i].secao.as_ref()).map_or(0, |s| s.numero_da_seguinte)).unwrap_or(0);
            if k > 0 {
                numero = if reinicia > 0 { reinicia as usize } else { numero + 1 };
            }
        }
        numero
    }

    /// A seção do cursor, para a aba Layout.
    pub fn secao_do_cursor(&self) -> InfoSecao {
        let k = self.idx(self.cursor.foco.bloco).map_or(0, |i| self.secao_do_bloco(i));
        let paginas: Vec<usize> = self.paginas.iter().enumerate().filter(|(_, p)| p.secao == k).map(|(n, _)| n).collect();
        let cfg = self.cfg_da_secao(k);
        let reinicia_em = match k {
            0 => 0,
            _ => self.dados_da_secao_anterior(k).map_or(0, |s| s.numero_da_seguinte),
        };
        let igual_a_anterior = k > 0 && self.faixas_da_secao(k) == self.faixas_da_secao(k - 1);
        InfoSecao {
            numero: k,
            total: self.total_de_secoes(),
            paginas: (paginas.first().copied().unwrap_or(0), paginas.last().copied().unwrap_or(0)),
            paisagem: cfg.largura > cfg.altura,
            reinicia_em,
            igual_a_anterior,
        }
    }

    /// Recompõe todos os blocos (o papel ou a numeração mudou).
    fn repaginar_tudo(&mut self) -> Mudanca {
        self.cache.clear();
        self.fim_da_edicao(Vec::new(), 0)
    }

    fn dados_da_secao_anterior(&self, k: usize) -> Option<&Secao> {
        k.checked_sub(1).and_then(|j| self.dados_da_secao(j))
    }

    /// Muda a seção `k` com `f`, num Ctrl+Z. A última seção é a do documento (papel e faixas dele);
    /// as outras guardam o que é delas na quebra que as fecha (criada a partir do documento).
    fn mudar_secao(&mut self, k: usize, f: impl FnOnce(&mut Secao)) -> Mudanca {
        let quebras = self.quebras_de_secao();
        let Some(&i) = quebras.get(k) else {
            let mut s = Secao { pagina: self.doc.pagina, cabecalho: self.doc.extras.cabecalho.clone(), rodape: self.doc.extras.rodape.clone(), numero_da_seguinte: 0 };
            f(&mut s);
            let antes = self.doc.extras.clone();
            self.doc.pagina = s.pagina;
            (self.doc.extras.cabecalho, self.doc.extras.rodape) = (s.cabecalho, s.rodape);
            if antes == self.doc.extras && s.pagina == self.cfg_da_secao(k) {
                return self.sem_mudanca();
            }
            // Como o papel e as margens do documento, não volta no Ctrl+Z (`papel_definir`).
            return self.repaginar_tudo();
        };
        let mut b = self.doc.blocos[i].clone();
        let mut s = b.secao.take().map_or_else(
            || Secao { pagina: self.doc.pagina, cabecalho: self.doc.extras.cabecalho.clone(), rodape: self.doc.extras.rodape.clone(), numero_da_seguinte: 0 },
            |s| *s,
        );
        f(&mut s);
        b.secao = Some(Box::new(s));
        if b == self.doc.blocos[i] {
            return self.sem_mudanca();
        }
        let cursor = self.cursor;
        self.transacao(crate::sessao::Splice { pos: i, quantos: 1, dentro: vec![b] }, cursor, None)
    }

    /// Retrato ou paisagem só na seção do cursor (o resto do documento fica como está).
    pub fn secao_orientacao_definir(&mut self, paisagem: bool) -> Mudanca {
        let k = self.secao_do_cursor().numero;
        self.mudar_secao(k, |s| {
            let (l, a) = (s.pagina.largura.min(s.pagina.altura), s.pagina.largura.max(s.pagina.altura));
            (s.pagina.largura, s.pagina.altura) = if paisagem { (a, l) } else { (l, a) };
        })
    }

    /// O cabeçalho e o rodapé da seção do cursor (os três campos de cada).
    pub fn secao_faixas_definir(&mut self, cabecalho: [&str; 3], rodape: [&str; 3]) -> Mudanca {
        let k = self.secao_do_cursor().numero;
        self.mudar_secao(k, |s| {
            for (f, t) in [(&mut s.cabecalho, cabecalho), (&mut s.rodape, rodape)] {
                (f.esquerda, f.centro, f.direita) = (t[0].to_string(), t[1].to_string(), t[2].to_string());
            }
        })
    }

    /// Os textos do cabeçalho e do rodapé da seção do cursor (para o diálogo).
    pub fn secao_faixas(&self) -> [String; 6] {
        let (c, r) = self.faixas_da_secao(self.secao_do_cursor().numero);
        [c.esquerda.clone(), c.centro.clone(), c.direita.clone(), r.esquerda.clone(), r.centro.clone(), r.direita.clone()]
    }

    /// A seção do cursor passa a ter o cabeçalho e o rodapé da de antes.
    pub fn secao_igual_a_anterior(&mut self) -> Mudanca {
        let k = self.secao_do_cursor().numero;
        if k == 0 {
            return self.sem_mudanca();
        }
        let (c, r) = self.faixas_da_secao(k - 1);
        let (c, r) = (c.clone(), r.clone());
        self.mudar_secao(k, |s| (s.cabecalho, s.rodape) = (c, r))
    }

    /// A seção do cursor recomeça a contar as páginas em `n` (0 = continua a contagem). Na primeira
    /// seção é o número inicial do documento.
    pub fn secao_numeracao_definir(&mut self, n: u32) -> Mudanca {
        let k = self.secao_do_cursor().numero;
        if k == 0 {
            let antes = self.doc.extras.pagina_inicial;
            self.doc.extras.pagina_inicial = n.max(1);
            if antes == self.doc.extras.pagina_inicial {
                return self.sem_mudanca();
            }
            return self.repaginar_tudo();
        }
        self.mudar_secao(k - 1, |s| s.numero_da_seguinte = n)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{Documento, InicioDeSecao, Trecho};

    /// Três parágrafos com uma quebra de seção (próxima página) no meio.
    fn com_duas_secoes() -> Sessao {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("um")] });
        d.acrescentar(Conteudo::QuebraDeSecao { inicio: InicioDeSecao::ProximaPagina });
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("dois")] });
        Sessao::com_documento(d)
    }

    #[test]
    fn paisagem_so_na_secao_do_cursor_com_paginas_de_papel_proprio() {
        let mut s = com_duas_secoes();
        s.ir_para_bloco(s.documento().blocos[0].id);
        s.secao_orientacao_definir(true);
        assert_eq!(s.total_de_paginas(), 2);
        let (p0, p1) = (s.cfg_da_pagina(0), s.cfg_da_pagina(1));
        assert!(p0.largura > p0.altura, "a primeira seção ficou deitada");
        assert!(p1.largura < p1.altura, "a segunda continua em pé");
        assert_eq!(s.secao_do_cursor().numero, 0);
        assert!(s.secao_do_cursor().paisagem);
        s.desfazer();
        assert!(s.cfg_da_pagina(0).largura < s.cfg_da_pagina(0).altura);
    }

    #[test]
    fn numeracao_reinicia_na_segunda_secao_e_cabecalho_proprio() {
        let mut s = com_duas_secoes();
        let segunda = s.documento().blocos[2].id;
        s.ir_para_bloco(segunda);
        assert_eq!((s.numero_da_pagina(0), s.numero_da_pagina(1)), (1, 2));
        s.secao_numeracao_definir(10);
        assert_eq!((s.numero_da_pagina(0), s.numero_da_pagina(1)), (1, 10));
        assert_eq!(s.secao_do_cursor().reinicia_em, 10);
        // Cabeçalho só da segunda seção (a última é a do documento); a primeira guarda o seu.
        s.ir_para_bloco(s.documento().blocos[0].id);
        s.secao_faixas_definir(["Capítulo 1", "", ""], ["", "", ""]);
        s.ir_para_bloco(segunda);
        assert!(!s.secao_do_cursor().igual_a_anterior);
        assert_eq!(s.secao_faixas()[0], "");
        s.secao_igual_a_anterior();
        assert_eq!(s.secao_faixas()[0], "Capítulo 1");
        assert!(s.secao_do_cursor().igual_a_anterior);
    }
}

#[cfg(test)]
mod testes_contorno {
    use crate::layout::Deco;
    use crate::modelo::{Conteudo, Documento, Flutuante, PosicaoFlutuante, Trecho};
    use crate::sessao::Sessao;

    #[test]
    fn imagem_solta_do_word_tem_o_texto_ao_lado_e_o_paragrafo_cresce_ate_ela() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Imagem { origem: "data:image/png;base64,AA==".into(), legenda: Vec::new(), largura: 100.0, altura: 80.0 });
        d.blocos[0].imagem.flutuante = Some(Flutuante { quebra: "square".into(), horizontal: PosicaoFlutuante { relativo: "column".into(), alinhar: "left".into(), deslocamento: 0.0 }, ..Default::default() });
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples(&"palavra ".repeat(200))] });
        let s = Sessao::com_documento(d);
        let p = &s.layouts[1];
        assert_eq!(s.layouts[0].linhas.iter().map(|l| l.altura).sum::<f32>(), 0.0, "o bloco da imagem não ocupa linha");
        assert!(p.linhas[0].deco.iter().any(|d| matches!(d, Deco::Imagem { w, .. } if *w == 100.0)));
        assert!(p.linhas[0].x0 >= 109.0, "as primeiras linhas começam depois da imagem e da folga");
        assert!(p.linhas.last().unwrap().x0 < 1.0, "as de baixo voltam à margem");
        // Um parágrafo curto cresce até a altura da imagem.
        let mut d = s.documento().clone();
        d.blocos[1].conteudo = Conteudo::Paragrafo { texto: vec![Trecho::simples("curto")] };
        let s = Sessao::com_documento(d);
        assert!(s.layouts[1].linhas.iter().map(|l| l.altura).sum::<f32>() >= 89.0);
    }
}
