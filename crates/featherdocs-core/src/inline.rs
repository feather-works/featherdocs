//! Texto em linha: a "fonte" Markdown de um bloco (`**negrito**`, `*itálico*`,
//! `` `código` ``, `~~tachado~~`, `++sublinhado++`, `$fórmula$`, `[texto](url)`)
//! para trechos e de volta. É o que o usuário digita ao editar um bloco.

use crate::modelo::{Marca, Trecho, CODIGO, ITALICO, MATEMATICA, NEGRITO, SUBLINHADO, TACHADO};

const MARCAS: [(&str, u8); 4] = [("**", NEGRITO), ("~~", TACHADO), ("++", SUBLINHADO), ("*", ITALICO)];

fn juntar(saida: &mut Vec<Trecho>, texto: &str, estilo: u8, link: &Option<String>) {
    if texto.is_empty() {
        return;
    }
    if let Some(ultimo) = saida.last_mut() {
        if ultimo.estilo == estilo && ultimo.link == *link {
            ultimo.texto.push_str(texto);
            return;
        }
    }
    saida.push(Trecho { texto: texto.to_string(), estilo, link: link.clone(), ..Trecho::default() });
}

/// Acha `fim` em `s` a partir do começo; só vale se não for logo no início.
fn fecha(s: &str, fim: &str) -> Option<usize> {
    s.find(fim).filter(|&i| i > 0)
}

/// Uma marca só abre se vem colada no texto e há onde fechar (também colada): `2 * 3` não é itálico.
fn pode_abrir(depois: &str, marca: &str) -> bool {
    if depois.chars().next().map_or(true, char::is_whitespace) {
        return false;
    }
    let mut de = 0;
    while let Some(i) = depois[de..].find(marca) {
        let pos = de + i;
        if pos > 0 && !depois[..pos].ends_with(char::is_whitespace) {
            return true;
        }
        de = pos + marca.len();
    }
    false
}

/// Lê um `[texto](url)` no começo de `s`; devolve (texto, url, bytes consumidos).
fn ler_link(s: &str) -> Option<(&str, &str, usize)> {
    let resto = s.strip_prefix('[')?;
    let fim_texto = resto.find("](")?;
    let depois = &resto[fim_texto + 2..];
    let fim_url = depois.find(')')?;
    Some((&resto[..fim_texto], &depois[..fim_url], 1 + fim_texto + 2 + fim_url + 1))
}

/// Analisa a fonte e devolve os trechos (adjacentes iguais já juntos).
pub fn analisar(fonte: &str) -> Vec<Trecho> {
    let mut saida = Vec::new();
    analisar_com(fonte, 0, &None, &mut saida);
    saida
}

fn analisar_com(fonte: &str, estilo_base: u8, link: &Option<String>, saida: &mut Vec<Trecho>) {
    let mut estilo = estilo_base;
    let mut texto = String::new();
    let mut resto = fonte;
    while let Some(c) = resto.chars().next() {
        if let Some(n) = passo(resto, c, &mut estilo, estilo_base, link, &mut texto, saida) {
            resto = &resto[n..];
        } else {
            texto.push(c);
            resto = &resto[c.len_utf8()..];
        }
    }
    juntar(saida, &texto, estilo, link);
}

/// Tenta tratar uma marca no começo de `resto`; devolve quantos bytes consumiu.
fn passo(resto: &str, c: char, estilo: &mut u8, base: u8, link: &Option<String>, texto: &mut String, saida: &mut Vec<Trecho>) -> Option<usize> {
    if c == '\\' {
        let proximo = resto[1..].chars().next().filter(|p| "\\*`$[]~+".contains(*p))?;
        texto.push(proximo);
        return Some(1 + proximo.len_utf8());
    }
    if c == '`' || c == '$' {
        let fim = fecha(&resto[1..], &c.to_string())?;
        juntar(saida, texto, *estilo, link);
        texto.clear();
        let marca = if c == '`' { CODIGO } else { MATEMATICA };
        juntar(saida, &resto[1..1 + fim], (*estilo & !(CODIGO | MATEMATICA)) | marca, link);
        return Some(fim + 2);
    }
    if c == '[' && link.is_none() {
        let (t, url, n) = ler_link(resto)?;
        juntar(saida, texto, *estilo, link);
        texto.clear();
        analisar_com(t, *estilo, &Some(url.to_string()), saida);
        return Some(n);
    }
    for (marca, bit) in MARCAS {
        if let Some(depois) = resto.strip_prefix(marca) {
            if *estilo & bit == 0 && !pode_abrir(depois, marca) {
                continue;
            }
            juntar(saida, texto, *estilo, link);
            texto.clear();
            *estilo ^= bit;
            *estilo = (*estilo & (NEGRITO | ITALICO | SUBLINHADO | TACHADO)) | (base & !(NEGRITO | ITALICO | SUBLINHADO | TACHADO));
            return Some(marca.len());
        }
    }
    None
}

fn escapar(texto: &str, saida: &mut String) {
    for c in texto.chars() {
        if "\\*`$[]~+".contains(c) {
            saida.push('\\');
        }
        saida.push(c);
    }
}

/// Os trechos como fonte, o inverso de `analisar`.
pub fn serializar(trechos: &[Trecho]) -> String {
    let mut saida = String::new();
    let mut ativo = 0u8;
    for t in trechos {
        if t.marca.is_some() {
            saida.push_str(&objeto_markdown(t));
            continue;
        }
        let quer = t.estilo & (NEGRITO | ITALICO | SUBLINHADO | TACHADO);
        for (marca, bit) in MARCAS.iter().rev() {
            if ativo & bit != 0 && quer & bit == 0 {
                saida.push_str(marca);
            }
        }
        for (marca, bit) in MARCAS {
            if ativo & bit == 0 && quer & bit != 0 {
                saida.push_str(marca);
            }
        }
        ativo = quer;
        corpo(t, &mut saida);
    }
    for (marca, bit) in MARCAS.iter().rev() {
        if ativo & bit != 0 {
            saida.push_str(marca);
        }
    }
    saida
}

/// Um objeto preso ao texto em Markdown: a nota vira a chamada `[^rótulo]` (a de fim, `[^fim-rótulo]`;
/// o texto vai nas definições do fim do arquivo), a citação vira `[@chave, p. 12]` (o jeito do Pandoc),
/// o campo vira o valor de agora e a entrada de índice some (o Markdown não tem índice).
pub fn objeto_markdown(t: &Trecho) -> String {
    match t.marca.as_deref() {
        Some(Marca::Nota { fim, .. }) => format!("[^{}{}]", if *fim { "fim-" } else { "" }, if t.rotulo.is_empty() { "nota" } else { &t.rotulo }),
        Some(Marca::Citacao { chaves, pagina }) => {
            let lista = chaves.iter().map(|c| format!("@{c}")).collect::<Vec<_>>().join("; ");
            if pagina.is_empty() {
                format!("[{lista}]")
            } else {
                format!("[{lista}, p. {pagina}]")
            }
        }
        Some(Marca::Campo { .. }) => t.rotulo.clone(),
        Some(Marca::ObjetoWord { resumo, .. }) => format!("<!-- {resumo} do Word: não exportado -->"),
        Some(Marca::Indice { .. } | Marca::Comentario { .. } | Marca::Imagem { .. } | Marca::Controle { .. }) | None => String::new(),
    }
}

fn corpo(t: &Trecho, saida: &mut String) {
    let mut dentro = String::new();
    if t.estilo & CODIGO != 0 {
        dentro = format!("`{}`", t.texto);
    } else if t.estilo & MATEMATICA != 0 {
        dentro = format!("${}$", t.texto);
    } else {
        escapar(&t.texto, &mut dentro);
    }
    match &t.link {
        Some(url) => saida.push_str(&format!("[{dentro}]({url})")),
        None => saida.push_str(&dentro),
    }
}

/// O texto puro, sem formatação e sem os objetos presos ao texto (para contar palavras, sumário, exportar).
pub fn texto_puro(trechos: &[Trecho]) -> String {
    trechos.iter().filter(|t| t.marca.is_none()).map(|t| t.texto.as_str()).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn volta(fonte: &str) {
        let t = analisar(fonte);
        assert_eq!(serializar(&t), fonte, "{t:?}");
        assert_eq!(analisar(&serializar(&t)), t);
    }

    #[test]
    fn marcas_basicas() {
        let t = analisar("a **b** *c* `d` ~~e~~ $x^2$");
        let estilos: Vec<u8> = t.iter().map(|x| x.estilo).collect();
        assert_eq!(estilos, vec![0, NEGRITO, 0, ITALICO, 0, CODIGO, 0, TACHADO, 0, MATEMATICA]);
        assert_eq!(texto_puro(&t), "a b c d e x^2");
    }

    #[test]
    fn ida_e_volta() {
        for f in ["simples", "a **negrito** e *itálico*", "**a *b* c**", "[ver](http://x.org) fim", "preço \\$5 e \\*estrela\\*", "`cod **x**` fim", "$\\alpha$ e mais"] {
            volta(f);
        }
    }

    #[test]
    fn marca_sem_fechar_vira_texto() {
        assert_eq!(texto_puro(&analisar("2 * 3 e **solto")), "2 * 3 e **solto");
    }

    #[test]
    fn link_com_negrito_dentro() {
        let t = analisar("[**oi**](u)");
        assert_eq!(t[0].estilo, NEGRITO);
        assert_eq!(t[0].link.as_deref(), Some("u"));
    }
}
