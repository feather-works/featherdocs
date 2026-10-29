//! Revisão → Comparar: o que mudou de uma versão (`antigo`) para a de agora, mostrado como
//! sugestões — o que saiu riscado, o que entrou sublinhado — para aceitar ou rejeitar uma a uma.
//!
//! Os parágrafos iguais servem de âncora (a maior sequência comum, em ordem); entre duas âncoras,
//! os parágrafos dos dois lados são comparados aos pares, palavra a palavra; o que sobra de um lado
//! foi apagado ou inserido inteiro.

use crate::inline::texto_puro;
use crate::modelo::{Bloco, Revisao, TipoDeRevisao, Trecho};
use crate::sessao::{Mudanca, Sessao};
use crate::texto::{fatia, juntar, normalizar, plano};

/// A maior sequência comum entre `a` e `b` (os pares de índices iguais, em ordem).
fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut v) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            v.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    v
}

/// As palavras (e os espaços e a pontuação entre elas) de um texto, com o começo de cada uma em caracteres.
fn palavras(s: &str) -> Vec<(usize, String)> {
    let mut v: Vec<(usize, String)> = Vec::new();
    let mut atual = String::new();
    let mut inicio = 0;
    let mut letra_antes: Option<bool> = None;
    for (k, c) in s.chars().enumerate() {
        let letra = c.is_alphanumeric();
        if letra_antes.is_some_and(|l| l != letra || !letra) && !atual.is_empty() {
            v.push((inicio, std::mem::take(&mut atual)));
            inicio = k;
        }
        atual.push(c);
        letra_antes = Some(letra);
    }
    if !atual.is_empty() {
        v.push((inicio, atual));
    }
    v
}

fn marcar(t: Vec<Trecho>, rev: &Revisao) -> Vec<Trecho> {
    t.into_iter()
        .map(|mut x| {
            if x.rev.is_none() {
                x.rev = Some(Box::new(rev.clone()));
            }
            x
        })
        .collect()
}

/// O texto `novo` com o que mudou desde o `antigo`, palavra a palavra.
pub fn comparar_texto(antigo: &[Trecho], novo: &[Trecho], autor: &str, quando: &str) -> Vec<Trecho> {
    let rev = |tipo| Revisao { tipo, autor: autor.to_string(), quando: quando.to_string(), movido: false };
    let (pa, pn) = (palavras(&plano(antigo)), palavras(&plano(novo)));
    let ta: Vec<&str> = pa.iter().map(|(_, w)| w.as_str()).collect();
    let tn: Vec<&str> = pn.iter().map(|(_, w)| w.as_str()).collect();
    let comuns = lcs(&ta, &tn);
    let fim_a = |k: usize| pa.get(k).map_or(plano(antigo).chars().count(), |(i, _)| *i);
    let fim_n = |k: usize| pn.get(k).map_or(plano(novo).chars().count(), |(i, _)| *i);
    let mut saida = Vec::new();
    let (mut i, mut j) = (0, 0);
    for (ci, cj) in comuns.into_iter().chain(std::iter::once((pa.len(), pn.len()))) {
        if ci > i {
            saida = juntar(saida, marcar(fatia(antigo, fim_a(i), fim_a(ci)), &rev(TipoDeRevisao::Remocao)));
        }
        if cj > j {
            saida = juntar(saida, marcar(fatia(novo, fim_n(j), fim_n(cj)), &rev(TipoDeRevisao::Insercao)));
        }
        if ci < pa.len() {
            saida = juntar(saida, fatia(novo, fim_n(cj), fim_n(cj + 1)));
        }
        (i, j) = (ci + 1, cj + 1);
    }
    normalizar(&mut saida);
    saida
}

/// Um bloco inteiro como apagado (ou inserido).
fn bloco_marcado(b: &Bloco, tipo: TipoDeRevisao, autor: &str, quando: &str) -> Bloco {
    let mut b = b.clone();
    let rev = Revisao { tipo, autor: autor.to_string(), quando: quando.to_string(), movido: false };
    for t in b.conteudo.textos_mut() {
        *t = marcar(std::mem::take(t), &rev);
    }
    b
}

/// O texto de um bloco para achar os iguais.
fn chave(b: &Bloco) -> (String, String) {
    (crate::edicao::nome_do_tipo(&b.conteudo), b.conteudo.textos().iter().map(|t| texto_puro(t)).collect::<Vec<_>>().join("\u{1f}"))
}

/// Os blocos do documento novo com o que mudou desde o antigo marcado. Os apagados vêm com `id` 0
/// (quem recebe dá um novo).
pub fn comparar_blocos(antigo: &[Bloco], novo: &[Bloco], autor: &str, quando: &str) -> Vec<Bloco> {
    let (ka, kn): (Vec<_>, Vec<_>) = (antigo.iter().map(chave).collect(), novo.iter().map(chave).collect());
    let ancoras = lcs(&ka, &kn);
    let mut saida = Vec::new();
    let (mut i, mut j) = (0, 0);
    for (ai, nj) in ancoras.into_iter().chain(std::iter::once((antigo.len(), novo.len()))) {
        let (va, vn) = (&antigo[i..ai], &novo[j..nj]);
        let pares = va.len().min(vn.len());
        for k in 0..pares {
            let (a, n) = (&va[k], &vn[k]);
            let (ta, tn) = (a.conteudo.textos(), n.conteudo.textos());
            if ka[i + k].0 == kn[j + k].0 && ta.len() == tn.len() {
                let mut b = n.clone();
                for (destino, (x, y)) in b.conteudo.textos_mut().into_iter().zip(ta.iter().zip(&tn)) {
                    *destino = comparar_texto(x, y, autor, quando);
                }
                saida.push(b);
            } else {
                saida.push(Bloco { id: 0, ..bloco_marcado(a, TipoDeRevisao::Remocao, autor, quando) });
                saida.push(bloco_marcado(n, TipoDeRevisao::Insercao, autor, quando));
            }
        }
        saida.extend(va[pares..].iter().map(|a| Bloco { id: 0, ..bloco_marcado(a, TipoDeRevisao::Remocao, autor, quando) }));
        saida.extend(vn[pares..].iter().map(|n| bloco_marcado(n, TipoDeRevisao::Insercao, autor, quando)));
        if ai < antigo.len() {
            saida.push(novo[nj].clone());
        }
        (i, j) = (ai + 1, nj + 1);
    }
    saida
}

impl Sessao {
    /// Revisão → Comparar: marca no documento aberto o que mudou desde `antigo` (um passo de
    /// desfazer). `rotulo` é o "autor" das marcas ("Comparação").
    pub fn comparar_com(&mut self, antigo: &crate::modelo::Documento, rotulo: &str) -> Mudanca {
        let blocos = comparar_blocos(&antigo.blocos, &self.doc.blocos, rotulo, &self.hoje);
        self.sem_rastrear(|s| s.trocar_blocos(blocos, None, None))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::inline::analisar;
    use crate::modelo::{Conteudo, Documento};

    fn marcado(t: &[Trecho]) -> String {
        t.iter()
            .map(|x| match x.rev.as_deref().map(|r| r.tipo) {
                Some(TipoDeRevisao::Insercao) => format!("[+{}]", x.texto),
                Some(TipoDeRevisao::Remocao) => format!("[-{}]", x.texto),
                None => x.texto.clone(),
            })
            .collect()
    }

    #[test]
    fn palavra_a_palavra() {
        let t = comparar_texto(&analisar("o gato preto dormiu"), &analisar("o gato branco dormiu muito"), "C", "");
        assert_eq!(marcado(&t), "o gato [-preto][+branco] dormiu[+ muito]");
    }

    #[test]
    fn paragrafos_apagados_inseridos_e_mudados() {
        let p = |s: &str| Bloco::novo(0, Conteudo::Paragrafo { texto: analisar(s) });
        let antigo = vec![p("um"), p("dois"), p("tres")];
        let mut novo = vec![p("um"), p("dois e meio"), p("quatro"), p("tres")];
        for (k, b) in novo.iter_mut().enumerate() {
            b.id = k as u64 + 10;
        }
        let r = comparar_blocos(&antigo, &novo, "C", "");
        let textos: Vec<String> = r.iter().map(|b| marcado(b.conteudo.textos()[0])).collect();
        assert_eq!(textos, ["um", "dois[+ e meio]", "[+quatro]", "tres"]);
        let r = comparar_blocos(&novo, &antigo, "C", "");
        let textos: Vec<String> = r.iter().map(|b| marcado(b.conteudo.textos()[0])).collect();
        assert_eq!(textos, ["um", "dois[- e meio]", "[-quatro]", "tres"]);
        assert_eq!(r[2].id, 0, "o apagado ganha id novo");
    }

    #[test]
    fn comparar_na_sessao_e_aceitar_tudo_volta_ao_novo() {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: analisar("texto novo aqui") });
        let mut s = Sessao::com_documento(d.clone());
        let mut antigo = Documento::vazio();
        antigo.acrescentar(Conteudo::Paragrafo { texto: analisar("texto velho aqui") });
        s.comparar_com(&antigo, "Comparação");
        assert_eq!(s.revisoes().len(), 2);
        s.revisoes_resolver(true, true);
        assert_eq!(texto_puro(s.documento().blocos[0].conteudo.textos()[0]), "texto novo aqui");
    }
}
