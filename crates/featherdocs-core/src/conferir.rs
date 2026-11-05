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
    /// Regras da ABNT (NBR 14724), só com elas pedidas ([`Sessao::conferir_abnt`]): o papel não é A4;
    /// as margens (`detalhe` = "sup/dir/inf/esq" em cm); parágrafos do texto em outro tamanho de
    /// fonte ou outra entrelinha (`detalhe` = o valor; `vezes` = quantos); citações longas sem o recuo de 4 cm.
    AbntPapel,
    AbntMargens,
    AbntFonte,
    AbntEntrelinha,
    AbntCitacao,
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

/// Um centímetro em pontos.
const CM: f32 = 72.0 / 2.54;

impl Sessao {
    /// O que a ABNT (NBR 14724) pede e o documento não segue: papel A4, margens de 3 cm em cima e à
    /// esquerda e 2 cm embaixo e à direita, fonte 12 e entrelinha 1,5 no texto, e recuo de 4 cm na
    /// citação longa (mais de três linhas). Cada faculdade muda alguma regra: por isso só com a
    /// opção ligada no painel, e cada item diz o que achou, sem consertar sozinho.
    pub fn conferir_abnt(&self) -> Vec<Pendencia> {
        let mut v = Vec::new();
        let p = &self.doc.pagina;
        let perto = |a: f32, b: f32| (a - b).abs() < 0.06 * CM;
        let (lado_a, lado_b) = (p.largura.min(p.altura), p.largura.max(p.altura));
        if !(perto(lado_a, 21.0 * CM) && perto(lado_b, 29.7 * CM)) {
            v.push(Pendencia { tipo: TipoDePendencia::AbntPapel, detalhe: String::new(), vezes: 1, lugar: None });
        }
        let margens = [p.margem_sup, p.margem_dir, p.margem_inf, p.margem_esq];
        if !margens.iter().zip([3.0, 2.0, 2.0, 3.0]).all(|(m, cm)| perto(*m, cm * CM)) {
            let detalhe = margens.iter().map(|m| format!("{:.1}", m / CM).replace('.', ",")).collect::<Vec<_>>().join(" / ");
            v.push(Pendencia { tipo: TipoDePendencia::AbntMargens, detalhe, vezes: 1, lugar: None });
        }
        // O texto: os parágrafos comuns com texto, sem as legendas (que a ABNT quer menores).
        let mut fontes: Vec<(String, usize, Option<(Pos, Pos)>)> = Vec::new();
        let mut entrelinhas: Vec<(String, usize, Option<(Pos, Pos)>)> = Vec::new();
        let mut citacoes = (0, None);
        let somar = |lista: &mut Vec<(String, usize, Option<(Pos, Pos)>)>, valor: String, lugar| match lista.iter_mut().find(|(v, ..)| *v == valor) {
            Some(x) => x.1 += 1,
            None => lista.push((valor, 1, lugar)),
        };
        for (i, b) in self.doc.blocos.iter().enumerate() {
            let estilo = self.doc.estilos.resolvido_do_bloco(b);
            let lugar = Some((Pos { bloco: b.id, celula: None, desl: 0 }, Pos { bloco: b.id, celula: None, desl: crate::alvo::tamanho_alvo(&b.conteudo, None) }));
            match &b.conteudo {
                crate::modelo::Conteudo::Paragrafo { texto } if b.atrib.legenda == 0 && texto.iter().any(|t| t.marca.is_none() && !t.texto.trim().is_empty()) => {
                    let corpo = estilo.as_ref().filter(|e| e.tamanho > 0.0).map_or_else(|| crate::layout::tamanho_base(&b.conteudo), |e| e.tamanho);
                    // O tamanho que vale é o da maior parte do texto (o trecho pode ter o seu).
                    let mut por_tamanho: Vec<(f32, usize)> = Vec::new();
                    for t in texto.iter().filter(|t| t.marca.is_none()) {
                        let tam = if t.fmt.tamanho > 0.0 { t.fmt.tamanho } else { corpo };
                        let n = t.texto.chars().filter(|c| !c.is_whitespace()).count();
                        match por_tamanho.iter_mut().find(|(x, _)| (*x - tam).abs() < 0.01) {
                            Some(x) => x.1 += n,
                            None => por_tamanho.push((tam, n)),
                        }
                    }
                    let tamanho = por_tamanho.iter().max_by_key(|(_, n)| *n).map_or(corpo, |(t, _)| *t);
                    if (tamanho - 12.0).abs() > 0.01 {
                        somar(&mut fontes, format!("{tamanho}").replace('.', ","), lugar);
                    }
                    let entrelinha = [b.atrib.entrelinha, estilo.as_ref().map_or(0.0, |e| e.entrelinha)].into_iter().find(|e| *e > 0.0).unwrap_or(1.15);
                    if (entrelinha - 1.5).abs() > 0.01 && b.atrib.mais.entrelinha_exata == 0.0 {
                        somar(&mut entrelinhas, format!("{entrelinha}").replace('.', ","), lugar);
                    }
                }
                crate::modelo::Conteudo::Citacao { .. } => {
                    let linhas = self.layouts.get(i).map_or(0, |l| l.linhas.len());
                    let recuo = b.atrib.recuo_esq + estilo.as_ref().map_or(0.0, |e| e.recuo_esq) + b.latex.parse::<f32>().unwrap_or(0.0);
                    if linhas > 3 && (recuo - 4.0 * CM).abs() > 0.1 * CM {
                        citacoes.0 += 1;
                        citacoes.1 = citacoes.1.or(lugar);
                    }
                }
                _ => {}
            }
        }
        for (tipo, lista) in [(TipoDePendencia::AbntFonte, fontes), (TipoDePendencia::AbntEntrelinha, entrelinhas)] {
            v.extend(lista.into_iter().map(|(detalhe, vezes, lugar)| Pendencia { tipo, detalhe, vezes, lugar }));
        }
        if citacoes.0 > 0 {
            v.push(Pendencia { tipo: TipoDePendencia::AbntCitacao, detalhe: String::new(), vezes: citacoes.0, lugar: citacoes.1 });
        }
        v
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
    #[test]
    fn regras_da_abnt_acusam_margem_fonte_e_entrelinha() {
        use crate::modelo::{Bloco, Conteudo, Documento, Trecho};
        let par = |id, t: &str| Bloco::novo(id, Conteudo::Paragrafo { texto: vec![Trecho::simples(t)] });
        let mut certo = par(2, "Texto certo.");
        certo.atrib.entrelinha = 1.5;
        let mut fonte = par(3, "Texto com fonte doze e entrelinha um e meio.");
        fonte.atrib.entrelinha = 1.5;
        let d = Documento { blocos: vec![par(1, "Texto padrão."), certo, fonte], ..Documento::default() };
        let mut s = Sessao::com_documento(d);
        let tipos = |s: &Sessao| s.conferir_abnt().into_iter().map(|p| (p.tipo, p.detalhe, p.vezes)).collect::<Vec<_>>();
        let achado = tipos(&s);
        assert!(achado.iter().any(|(t, d, _)| *t == TipoDePendencia::AbntMargens && d == "2,5 / 2,5 / 2,5 / 2,5"), "{achado:?}");
        assert!(achado.iter().any(|(t, d, n)| *t == TipoDePendencia::AbntEntrelinha && d == "1,15" && *n == 1), "só o parágrafo sem 1,5: {achado:?}");
        assert!(!achado.iter().any(|(t, ..)| *t == TipoDePendencia::AbntPapel), "A4 é o padrão");
        let cm = 72.0 / 2.54;
        s.margens_definir(3.0 * cm, 2.0 * cm, 3.0 * cm, 2.0 * cm).unwrap();
        assert!(!tipos(&s).iter().any(|(t, ..)| *t == TipoDePendencia::AbntMargens), "3/2/2/3 está certo");
    }

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
