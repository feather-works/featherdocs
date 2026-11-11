//! A ponte de tabela com o FeatherSheet: células em CSV (UTF-8, vírgula, aspas
//! duplas quando preciso). O FeatherDocs grava a tabela num arquivo temporário,
//! o FeatherSheet o abre e salva, e o FeatherDocs lê as células de volta. Nenhum
//! app importa código do outro: o contrato é o arquivo.

/// Células → texto CSV.
pub fn gravar(linhas: &[Vec<String>]) -> String {
    let mut s = String::new();
    for l in linhas {
        let cels: Vec<String> = l.iter().map(|c| campo(c)).collect();
        s.push_str(&cels.join(","));
        s.push('\n');
    }
    s
}

fn campo(c: &str) -> String {
    // O `;` também: o `ler` toma o separador pela primeira linha, e um `a;b;c` solto o trocaria.
    if c.contains([',', ';', '"', '\n', '\r']) {
        format!("\"{}\"", c.replace('"', "\"\""))
    } else {
        c.to_string()
    }
}

/// Texto CSV → células. Aceita `,` ou `;` (o que o FeatherSheet/Excel gravar em português) e aspas.
pub fn ler(texto: &str) -> Vec<Vec<String>> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let primeira = texto.lines().next().unwrap_or("");
    // Conta só fora das aspas: `"a;b;c",x` é vírgula.
    let (mut pv, mut v, mut aspas) = (0, 0, false);
    for c in primeira.chars() {
        match c {
            '"' => aspas = !aspas,
            ';' if !aspas => pv += 1,
            ',' if !aspas => v += 1,
            _ => {}
        }
    }
    let sep = if pv > v { ';' } else { ',' };
    let mut linhas: Vec<Vec<String>> = Vec::new();
    let mut linha: Vec<String> = Vec::new();
    let mut atual = String::new();
    let mut aspas = false;
    let mut cs = texto.chars().peekable();
    while let Some(c) = cs.next() {
        match (aspas, c) {
            (true, '"') if cs.peek() == Some(&'"') => {
                atual.push('"');
                cs.next();
            }
            (true, '"') => aspas = false,
            (true, _) => atual.push(c),
            (false, '"') => aspas = true,
            (false, c) if c == sep => linha.push(std::mem::take(&mut atual)),
            (false, '\r') => {}
            (false, '\n') => {
                linha.push(std::mem::take(&mut atual));
                linhas.push(std::mem::take(&mut linha));
            }
            (false, _) => atual.push(c),
        }
    }
    if !atual.is_empty() || !linha.is_empty() {
        linha.push(atual);
        linhas.push(linha);
    }
    linhas
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ida_e_volta_com_virgula_e_aspas() {
        let l = vec![vec!["a,b".to_string(), "diz \"oi\"".to_string()], vec!["1".to_string(), String::new()]];
        assert_eq!(ler(&gravar(&l)), l);
    }

    #[test]
    fn ponto_e_virgula_do_excel_brasileiro() {
        assert_eq!(ler("a;b\n1;2\n"), vec![vec!["a", "b"], vec!["1", "2"]]);
    }

    #[test]
    fn celula_com_ponto_e_virgula_volta_inteira() {
        let l = vec![vec!["a;b;c".to_string(), "x".to_string()]];
        assert_eq!(ler(&gravar(&l)), l);
    }
}
