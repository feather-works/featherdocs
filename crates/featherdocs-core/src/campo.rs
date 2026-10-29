//! Os campos do Word que o `Marca::Campo` guarda além dos que o FeatherDocs calcula (`autor`, `pagina`…):
//! a data com o formato do Word e o campo que só vai e volta (`MERGEFIELD`, `REF`, `=`, `DOCPROPERTY`…).

use crate::modelo::Marca;

/// O separador entre a instrução e o resultado guardado no `campo`.
const SEPARADOR: char = '\u{1f}';

/// O campo do Word que o FeatherDocs não calcula: a instrução e o resultado que o Word guardou
/// (que a tela mostra). `campo` = `word:<instrução>␟<resultado>`.
pub fn do_word(instr: &str, resultado: &str) -> Marca {
    Marca::Campo { campo: format!("word:{}{SEPARADOR}{resultado}", instr.trim()) }
}

/// A instrução e o resultado de um campo de [`do_word`].
pub fn partes_do_word(campo: &str) -> Option<(&str, &str)> {
    let resto = campo.strip_prefix("word:")?;
    Some(resto.split_once(SEPARADOR).unwrap_or((resto, "")))
}

/// O campo de data com o formato do Word (`d 'de' MMMM 'de' yyyy`); `data` sozinho é dd/MM/yyyy.
pub fn data_com_formato(formato: &str) -> String {
    if formato == "dd/MM/yyyy" { "data".into() } else { format!("data:{formato}") }
}

/// O formato de um campo de data (`data` = dd/MM/yyyy).
pub fn formato_da_data(campo: &str) -> Option<&str> {
    if campo == "data" { Some("dd/MM/yyyy") } else { campo.strip_prefix("data:") }
}

const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
const DIAS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];

/// A data `hoje` (dd/MM/yyyy, como a interface escreve) no formato do Word: `d`, `dd`, `ddd`, `dddd`,
/// `M`, `MM`, `MMM`, `MMMM`, `yy`, `yyyy` e texto entre aspas simples. Data ilegível volta como veio.
pub fn formatar_data(hoje: &str, formato: &str) -> String {
    let n: Vec<u32> = hoje.split('/').filter_map(|p| p.trim().parse().ok()).collect();
    let [d, m, a] = n[..] else { return hoje.to_string() };
    if !(1..=12).contains(&m) || d == 0 {
        return hoje.to_string();
    }
    // Zeller (0 = domingo).
    let (mm, aa) = if m < 3 { (m + 12, a - 1) } else { (m, a) };
    let semana = ((d + 13 * (mm + 1) / 5 + aa + aa / 4 - aa / 100 + aa / 400 + 6) % 7) as usize;
    let mut s = String::new();
    let c: Vec<char> = formato.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\'' {
            let fim = c[i + 1..].iter().position(|&x| x == '\'').map_or(c.len(), |p| i + 1 + p);
            s.extend(&c[i + 1..fim]);
            i = fim + 1;
            continue;
        }
        let n = c[i..].iter().take_while(|&&x| x == ch).count();
        match (ch, n) {
            ('d', 1) => s.push_str(&d.to_string()),
            ('d', 2) => s.push_str(&format!("{d:02}")),
            ('d', 3) => s.extend(DIAS[semana].chars().take(3)),
            ('d', _) => s.push_str(DIAS[semana]),
            ('M', 1) => s.push_str(&m.to_string()),
            ('M', 2) => s.push_str(&format!("{m:02}")),
            ('M', 3) => s.extend(MESES[m as usize - 1].chars().take(3)),
            ('M', _) => s.push_str(MESES[m as usize - 1]),
            ('y', 1 | 2) => s.push_str(&format!("{:02}", a % 100)),
            ('y', _) => s.push_str(&a.to_string()),
            _ => {
                s.extend(&c[i..i + n]);
            }
        }
        i += n;
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_no_formato_do_word_e_campo_guardado() {
        assert_eq!(formatar_data("29/10/2026", "d 'de' MMMM 'de' yyyy"), "29 de outubro de 2026");
        assert_eq!(formatar_data("01/03/2026", "dddd, dd/MM/yy"), "domingo, 01/03/26");
        assert_eq!(formatar_data("?", "dd/MM"), "?");
        assert_eq!(formato_da_data(&data_com_formato("MMMM")), Some("MMMM"));
        assert_eq!(formato_da_data("data"), Some("dd/MM/yyyy"));
        let Marca::Campo { campo } = do_word(" MERGEFIELD Nome ", "«Nome»") else { panic!() };
        assert_eq!(partes_do_word(&campo), Some(("MERGEFIELD Nome", "«Nome»")));
        assert_eq!(partes_do_word("autor"), None);
    }
}
