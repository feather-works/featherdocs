//! Conferir antes de entregar: o que ainda falta resolver no documento (referência perdida,
//! citação sem fonte, comentário aberto, alteração pendente, palavra errada, título vazio), cada
//! item com o lugar para onde o clique leva.

use crate::alvo::{texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::layout::texto_do_titulo;
use crate::modelo::Marca;
use crate::sessao::Sessao;

/// O tipo do que falta. A interface escreve a frase (na língua dela) com o `detalhe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum TipoDePendencia {
    /// Referência cruzada para um título, legenda ou marcador que sumiu.
    Referencia,
    /// Citação de uma chave que não está nas fontes (`detalhe` = a chave).
    CitacaoSemFonte,
    /// Fonte cadastrada que o texto nunca cita (`detalhe` = a chave).
    FonteNaoCitada,
    /// Comentário ainda aberto (`detalhe` = o texto dele).
    Comentario,
    /// Alterações sugeridas à espera de aceitar ou rejeitar (`detalhe` = quantas).
    Alteracao,
    /// Palavra que o dicionário não conhece (`detalhe` = a palavra; `vezes` = quantas vezes aparece).
    Ortografia,
    /// Deslize de digitação: palavra repetida, espaço duplo, espaço antes da pontuação (`detalhe` = o texto achado).
    Deslize,
    /// Título sem texto.
    TituloVazio,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Pendencia {
    pub tipo: TipoDePendencia,
    pub detalhe: String,
    pub vezes: usize,
    /// O trecho que o clique seleciona (início e fim); `None` = não tem lugar no texto.
    #[serde(skip)]
    pub lugar: Option<(Pos, Pos)>,
}

/// Mais que isto de palavras erradas diferentes, a lista corta (o resto aparece no sublinhado).
const MAXIMO_DE_PALAVRAS: usize = 50;

impl Sessao {
    /// O que falta resolver, na ordem: referências, citações, comentários, alterações, títulos e ortografia.
    pub fn conferir(&self) -> Vec<Pendencia> {
        let mut v = Vec::new();
        let chaves: Vec<&str> = self.doc.bibliografia.fontes.iter().map(|f| f.chave.as_str()).collect();
        let mut erros: Vec<Pendencia> = Vec::new();
        let mut titulos = Vec::new();
        for b in &self.doc.blocos {
            if texto_do_titulo(b).is_some_and(|(_, t)| t.trim().is_empty()) {
                let p = Pos { bloco: b.id, celula: None, desl: 0 };
                titulos.push(Pendencia { tipo: TipoDePendencia::TituloVazio, detalhe: String::new(), vezes: 1, lugar: Some((p, p)) });
            }
            for cel in crate::busca::alvos_do_bloco(&b.conteudo) {
                let Some(t) = texto_alvo(&b.conteudo, cel) else { continue };
                let em = |a: usize, z: usize| Some((Pos { bloco: b.id, celula: cel, desl: a }, Pos { bloco: b.id, celula: cel, desl: z }));
                let mut pos = 0;
                for tr in &t {
                    let n = tr.texto.chars().count();
                    if let Some((id, modo)) = tr.link.as_deref().and_then(crate::referencias::ler_ref) {
                        if self.texto_da_referencia(id, modo).is_none() {
                            v.push(Pendencia { tipo: TipoDePendencia::Referencia, detalhe: tr.texto.clone(), vezes: 1, lugar: em(pos, pos + n) });
                        }
                    }
                    if let Some(Marca::Citacao { chaves: citadas, .. }) = tr.marca.as_deref() {
                        for c in citadas.iter().filter(|c| !chaves.contains(&c.as_str())) {
                            v.push(Pendencia { tipo: TipoDePendencia::CitacaoSemFonte, detalhe: c.clone(), vezes: 1, lugar: em(pos, pos + n) });
                        }
                    }
                    pos += n;
                }
                if !matches!(b.conteudo, crate::modelo::Conteudo::Codigo { .. }) {
                    juntar_erros(&mut erros, &t, &self.doc.extras.documento.idioma, cel, b.id);
                }
            }
        }
        let citadas = crate::citacoes::chaves_citadas(&self.doc.blocos);
        for f in self.doc.bibliografia.fontes.iter().filter(|f| !citadas.contains(&f.chave)) {
            v.push(Pendencia { tipo: TipoDePendencia::FonteNaoCitada, detalhe: f.chave.clone(), vezes: 1, lugar: None });
        }
        for c in self.comentarios().into_iter().filter(|c| !c.resolvido) {
            let lugar = self.trecho_do_comentario(c.id).map(|(i, cel, a, z)| {
                let bloco = self.doc.blocos[i].id;
                (Pos { bloco, celula: cel, desl: a }, Pos { bloco, celula: cel, desl: z })
            });
            v.push(Pendencia { tipo: TipoDePendencia::Comentario, detalhe: c.texto, vezes: 1, lugar });
        }
        let revisoes = self.revisoes();
        if let Some(r) = revisoes.first() {
            let p = self.idx(r.bloco).map(|i| self.inicio_do_bloco(i));
            v.push(Pendencia { tipo: TipoDePendencia::Alteracao, detalhe: String::new(), vezes: revisoes.len(), lugar: p.map(|p| (p, p)) });
        }
        v.extend(titulos);
        v.extend(erros);
        v
    }

    /// Seleciona o lugar da pendência (o clique na lista). `false` se ela não tem lugar ou ele sumiu.
    pub fn pendencia_ir(&mut self, p: &Pendencia) -> bool {
        let Some((a, z)) = p.lugar else { return false };
        if self.idx(a.bloco).is_none() {
            return false;
        }
        self.cursor = Cursor { ancora: a, foco: z, x_desejado: None, pendente: None };
        true
    }
}

/// Soma as palavras erradas do `texto` à lista, uma entrada por palavra (com a primeira ocorrência).
fn juntar_erros(lista: &mut Vec<Pendencia>, texto: &[crate::modelo::Trecho], idioma: &str, cel: Cel, bloco: u64) {
    let Some(achadas) = crate::ortografia::erros_do_texto(texto, idioma) else { return };
    for (a, z, palavra) in achadas {
        if let Some(p) = lista.iter_mut().find(|p| p.detalhe == palavra) {
            p.vezes += 1;
        } else if lista.len() < MAXIMO_DE_PALAVRAS {
            let lugar = Some((Pos { bloco, celula: cel, desl: a }, Pos { bloco, celula: cel, desl: z }));
            let tipo = if crate::ortografia::correcao_do_deslize(&palavra).is_some() { TipoDePendencia::Deslize } else { TipoDePendencia::Ortografia };
            lista.push(Pendencia { tipo, detalhe: palavra, vezes: 1, lugar });
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{Bloco, Conteudo, Documento, Fonte, Trecho};

    #[test]
    fn acha_citacao_sem_fonte_fonte_nao_citada_e_titulo_vazio() {
        let citacao = Trecho { marca: Some(Box::new(Marca::Citacao { chaves: vec!["sumida".into()], pagina: String::new() })), ..Trecho::simples(crate::modelo::OBJETO.to_string().as_str()) };
        let mut doc = Documento {
            blocos: vec![
                Bloco::novo(1, Conteudo::Titulo { nivel: 1, texto: vec![] }),
                Bloco::novo(2, Conteudo::Paragrafo { texto: vec![Trecho::simples("Texto "), citacao] }),
            ],
            ..Documento::default()
        };
        doc.bibliografia.fontes.push(Fonte { chave: "silva2023".into(), ..Fonte::default() });
        let mut s = Sessao::com_documento(doc);
        let tipos: Vec<_> = s.conferir().iter().map(|p| (p.tipo, p.detalhe.clone())).collect();
        assert!(tipos.contains(&(TipoDePendencia::CitacaoSemFonte, "sumida".into())), "{tipos:?}");
        assert!(tipos.contains(&(TipoDePendencia::FonteNaoCitada, "silva2023".into())));
        assert!(tipos.iter().any(|(t, _)| *t == TipoDePendencia::TituloVazio));
        let citacao = s.conferir().into_iter().find(|p| p.tipo == TipoDePendencia::CitacaoSemFonte).unwrap();
        assert!(s.pendencia_ir(&citacao));
        assert_eq!((s.cursor().ancora.desl, s.cursor().foco.desl), (6, 7), "seleciona a citação");
    }

    #[test]
    fn referencia_para_bloco_que_sumiu() {
        let r = Trecho { link: Some("#ref:99:texto".into()), ..Trecho::simples("Erro! Referência não encontrada.") };
        let s = Sessao::com_documento(Documento { blocos: vec![Bloco::novo(1, Conteudo::Paragrafo { texto: vec![r] })], ..Documento::default() });
        assert_eq!(s.conferir().iter().filter(|p| p.tipo == TipoDePendencia::Referencia).count(), 1);
    }
}
