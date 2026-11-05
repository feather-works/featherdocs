//! Marcadores e referências cruzadas.
//!
//! Um marcador dá nome a um bloco. Uma referência cruzada é um texto com link interno
//! `#ref:<id>:<modo>` que mostra o texto, o número ou a página de um título, de uma legenda ou
//! de um marcador; "Atualizar referências" (o F9 do Word) refaz esses textos todos de uma vez.

use serde::Serialize;

use crate::alvo::{com_texto_ref, texto_alvo};
use crate::edicao::trechos_de;
use crate::inline::texto_puro;
use crate::modelo::{Conteudo, Marcador};
use crate::sessao::{Mudanca, Sessao};

/// Algo que uma referência cruzada pode apontar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlvoDeReferencia {
    /// `titulo`, `figura`, `tabela`, `equacao` ou `marcador`.
    pub tipo: String,
    pub id: u64,
    /// O nome do marcador, ou o texto do título/legenda.
    pub texto: String,
    /// "1.2", "Figura 3"; vazio se o alvo não tem número.
    pub numero: String,
    /// A página (1, 2…).
    pub pagina: usize,
}

/// Nome de marcador: letras, números e `_`, começando por letra (como no Word).
fn nome_valido(nome: &str) -> Result<String, String> {
    let n = nome.trim();
    if n.is_empty() || !n.chars().next().is_some_and(char::is_alphabetic) || !n.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("O nome do marcador começa por letra e só tem letras, números e _ (sem espaço).".to_string());
    }
    Ok(n.to_string())
}

/// `#ref:<id>:<modo>` → (id, modo).
pub(crate) fn ler_ref(link: &str) -> Option<(u64, &str)> {
    let resto = link.strip_prefix("#ref:")?;
    let (id, modo) = resto.split_once(':')?;
    Some((id.parse().ok()?, modo))
}

/// O `id` do bloco para onde um link interno leva (`#ref:…` ou `#bloco:<id>`).
pub fn destino_do_link(link: &str) -> Option<u64> {
    ler_ref(link).map(|(id, _)| id).or_else(|| link.strip_prefix("#bloco:")?.parse().ok())
}

impl Sessao {
    /// Inserir → Marcador: dá `nome` ao parágrafo do cursor (um nome que já existe muda de lugar).
    pub fn marcador_adicionar(&mut self, nome: &str) -> Result<Mudanca, String> {
        let nome = nome_valido(nome)?;
        let bloco = self.cursor.foco.bloco;
        self.doc.marcadores.retain(|m| m.nome != nome);
        self.doc.marcadores.push(Marcador { nome, bloco });
        self.doc.marcadores.sort_by_key(|m| m.nome.to_lowercase());
        Ok(self.fim_da_edicao_sem_blocos())
    }

    pub fn marcador_remover(&mut self, nome: &str) -> Mudanca {
        let antes = self.doc.marcadores.len();
        self.doc.marcadores.retain(|m| m.nome != nome);
        if antes == self.doc.marcadores.len() {
            return self.sem_mudanca();
        }
        self.fim_da_edicao_sem_blocos()
    }

    /// O texto puro de um bloco (título, parágrafo, legenda…), cortado se for comprido.
    fn texto_curto(&self, i: usize) -> String {
        let t = trechos_de(&self.doc.blocos[i].conteudo).map(|t| texto_puro(t)).unwrap_or_default();
        let t = t.trim();
        if t.chars().count() > 80 {
            format!("{}…", t.chars().take(79).collect::<String>())
        } else {
            t.to_string()
        }
    }

    fn numero_do_bloco(&self, i: usize) -> String {
        self.numeros.get(i).cloned().flatten().map(|n| n.trim_end_matches(" –").to_string()).unwrap_or_default()
    }

    /// Tudo o que a caixa de referência cruzada oferece: títulos, legendas e marcadores, na ordem do documento.
    pub fn alvos_de_referencia(&self) -> Vec<AlvoDeReferencia> {
        let pagina = |id: u64| self.pagina_do_bloco(id).map_or(0, |p| self.numero_da_pagina(p));
        let mut v: Vec<AlvoDeReferencia> = Vec::new();
        for (i, b) in self.doc.blocos.iter().enumerate() {
            let tipo = match (&b.conteudo, b.atrib.legenda) {
                (Conteudo::Titulo { .. }, _) => "titulo",
                (Conteudo::Paragrafo { .. }, 1) => "figura",
                (Conteudo::Paragrafo { .. }, 2) => "tabela",
                (Conteudo::Paragrafo { .. }, 3) => "equacao",
                _ => continue,
            };
            v.push(AlvoDeReferencia { tipo: tipo.to_string(), id: b.id, texto: self.texto_curto(i), numero: self.numero_do_bloco(i), pagina: pagina(b.id) });
        }
        for m in &self.doc.marcadores {
            if self.idx(m.bloco).is_some() {
                v.push(AlvoDeReferencia { tipo: "marcador".to_string(), id: m.bloco, texto: m.nome.clone(), numero: String::new(), pagina: pagina(m.bloco) });
            }
        }
        v
    }

    /// O que a referência mostra agora: `texto` (do alvo), `numero` ("1.2", "Figura 3") ou `pagina`.
    pub(crate) fn texto_da_referencia(&self, id: u64, modo: &str) -> Option<String> {
        let i = self.idx(id)?;
        Some(match modo {
            "pagina" => self.numero_da_pagina(self.pagina_do_bloco(id)?).to_string(),
            "numero" => {
                let n = self.numero_do_bloco(i);
                if n.is_empty() {
                    self.texto_curto(i)
                } else {
                    n
                }
            }
            _ => self.texto_curto(i),
        })
    }

    /// Referências → Referência cruzada: insere no cursor o texto, o número ou a página do bloco `id`, com o link para ele.
    pub fn referencia_inserir(&mut self, id: u64, modo: &str) -> Result<Mudanca, String> {
        if !matches!(modo, "texto" | "numero" | "pagina") {
            return Err(format!("Não conheço a referência \"{modo}\"."));
        }
        let texto = self.texto_da_referencia(id, modo).ok_or("O alvo da referência não existe mais.")?;
        if texto.is_empty() {
            return Err("O alvo da referência está vazio.".to_string());
        }
        Ok(self.inserir_com_link(&texto, &format!("#ref:{id}:{modo}")))
    }

    /// Refaz o texto de todas as referências cruzadas (um passo de desfazer). As que apontam para um bloco
    /// que sumiu mostram "Erro! Referência não encontrada.", como no Word.
    pub fn referencias_atualizar(&mut self) -> Mudanca {
        let mut trocas: Vec<(usize, crate::modelo::Bloco)> = Vec::new();
        for (i, b) in self.doc.blocos.iter().enumerate() {
            let celulas: Vec<crate::alvo::Cel> = match &b.conteudo {
                Conteudo::Tabela { linhas, .. } => (0..linhas.len()).flat_map(|l| (0..linhas[l].len()).map(move |c| Some((l as u32, c as u32)))).collect(),
                _ => vec![None],
            };
            let mut novo = b.clone();
            let mut mudou = false;
            for cel in celulas {
                let Some(t) = texto_alvo(&b.conteudo, cel) else { continue };
                if !t.iter().any(|tr| tr.link.as_deref().is_some_and(|l| l.starts_with("#ref:"))) {
                    continue;
                }
                let refeito: Vec<_> = t
                    .iter()
                    .map(|tr| {
                        let Some((id, modo)) = tr.link.as_deref().and_then(ler_ref) else { return tr.clone() };
                        let texto = self.texto_da_referencia(id, modo).unwrap_or_else(|| "Erro! Referência não encontrada.".to_string());
                        crate::modelo::Trecho { texto, ..tr.clone() }
                    })
                    .collect();
                if refeito != t {
                    com_texto_ref(&mut novo.conteudo, cel, |x| *x = refeito.clone());
                    mudou = true;
                }
            }
            if mudou {
                trocas.push((i, novo));
            }
        }
        let (Some(&(i0, _)), Some(&(i1, _))) = (trocas.first(), trocas.last()) else { return self.sem_mudanca() };
        let mut t = self.trabalho(i0, i1);
        for (i, b) in trocas {
            t.blocos[i - i0] = b;
        }
        let cursor = self.cursor;
        self.fechar_trabalho(t, cursor, None)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cursor::Pos;
    use crate::inline::analisar;
    use crate::modelo::Documento;

    fn doc() -> Sessao {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Introdução") });
        d.acrescentar(Conteudo::Paragrafo { texto: analisar("Veja ") });
        let mut s = Sessao::com_documento(d);
        s.numeracao_definir("decimal").unwrap();
        s
    }

    fn texto_do(s: &Sessao, i: usize) -> String {
        texto_puro(trechos_de(&s.documento().blocos[i].conteudo).unwrap())
    }

    #[test]
    fn referencia_ao_numero_e_atualizacao() {
        let mut s = doc();
        let alvo = s.documento().blocos[0].id;
        let p = s.documento().blocos[1].id;
        s.cursor = crate::cursor::Cursor::em(Pos { bloco: p, celula: None, desl: 5 });
        s.referencia_inserir(alvo, "numero").unwrap();
        assert_eq!(texto_do(&s, 1), "Veja 1");
        assert!(s.alvos_de_referencia().iter().any(|a| a.tipo == "titulo" && a.numero == "1"));
        // Um título novo antes: o número muda e "Atualizar" acompanha.
        let mut d = s.documento().clone();
        d.blocos.insert(0, crate::modelo::Bloco::novo(99, Conteudo::Titulo { nivel: 1, texto: analisar("Prefácio") }));
        let mut s = Sessao::com_documento(d);
        let m = s.referencias_atualizar();
        assert_eq!(m.afetados.len(), 1);
        assert_eq!(texto_do(&s, 2), "Veja 2");
        assert_eq!(destino_do_link(&format!("#ref:{alvo}:numero")), Some(alvo));
    }

    #[test]
    fn marcador_com_nome_valido_e_alvo_que_sumiu() {
        let mut s = doc();
        assert!(s.marcador_adicionar("tem espaço").is_err());
        s.marcador_adicionar("inicio").unwrap();
        assert!(s.alvos_de_referencia().iter().any(|a| a.tipo == "marcador" && a.texto == "inicio"));
        let alvo = s.documento().blocos[0].id;
        let p = s.documento().blocos[1].id;
        s.cursor = crate::cursor::Cursor::em(Pos { bloco: p, celula: None, desl: 5 });
        s.referencia_inserir(alvo, "pagina").unwrap();
        assert_eq!(texto_do(&s, 1), "Veja 1");
        let mut d = s.documento().clone();
        d.blocos.remove(0);
        let mut s = Sessao::com_documento(d);
        s.referencias_atualizar();
        assert!(texto_do(&s, 0).contains("Referência não encontrada"));
        s.marcador_remover("inicio");
        assert!(s.documento().marcadores.is_empty());
    }
}
