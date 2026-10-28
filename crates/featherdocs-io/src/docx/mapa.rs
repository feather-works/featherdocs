//! O mapa único das propriedades de caractere (`w:rPr`) e de parágrafo (`w:pPr`) do `.docx`: cada
//! entrada diz o elemento e o atributo do OOXML, o campo do modelo e a conversão. O leitor
//! (`docx.rs`) e o gravador (`docx_gravar.rs`) percorrem as mesmas tabelas: uma propriedade nova é
//! uma linha aqui, e ela já vai e volta. As entradas de um mesmo elemento ficam juntas, e os
//! elementos na ordem que o esquema do Word exige.
//!
//! Fora do mapa, escritos à parte: `w:rStyle` (o link), `w:rFonts` (o nome da fonte vira os bits
//! `SERIFA`/`CODIGO` por semelhança, não é uma conversão de valor), `w:pStyle` e `w:numPr` (apontam
//! estilos e numerações) e `w:pBdr` (quatro elementos filhos numa máscara de bits, com uma cor só).

use featherdocs_core::modelo::{Alinhamento, Atrib, Fmt, ITALICO, NEGRITO, SUBLINHADO, TACHADO};
use quick_xml::events::BytesStart;

use super::{atributo, cor, cor_do_realce, ligado, numero};

/// O caractere: os bits de `Trecho::estilo` e o `Fmt`.
pub(crate) type Car = (u8, Fmt);

/// Como o valor do atributo vira o campo do modelo, e volta.
pub(crate) enum Conv<T> {
    /// Liga/desliga um bit (`w:val="0"`, `false`, `none`, `off` desliga). Grava este valor no
    /// atributo; vazio = o elemento sem atributo (`<w:b/>`).
    Bit(fn(&mut T) -> &mut u8, u8, &'static str),
    /// Medida em pontos; o arquivo guarda esta quantidade por ponto (vigésimos: 20, meios-pontos: 2).
    /// Ausente = 0.
    Medida(fn(&mut T) -> &mut f32, f32),
    /// Como `Medida`, opcional: ausente não mexe (`None` = a do tipo de bloco).
    Opcional(fn(&mut T) -> &mut Option<f32>, f32),
    /// `RRGGBB` ↔ `0xRRGGBBFF`; `auto` e ausente = 0.
    Cor(fn(&mut T) -> &mut u32),
    /// Só leitura: as cores com nome do `w:highlight` (o gravador usa o `w:shd`).
    CorComNome(fn(&mut T) -> &mut u32),
    /// Um valor de uma lista: a função lê o código (e o troca, com `Some`). O código 0 é o padrão,
    /// que não se grava; o primeiro nome de cada código é o gravado, os outros são sinônimos.
    Opcoes(fn(&mut T, Option<i8>) -> i8, &'static [(&'static str, i8)]),
    /// Entrelinha múltipla: `w:line` em 240 avos, só com `w:lineRule="auto"` (ou sem ele); ausente não mexe.
    Entrelinha(fn(&mut T) -> &mut f32),
    /// Recuo da primeira linha em pontos: `w:firstLine` positivo, `w:hanging` negativo.
    PrimeiraLinha(fn(&mut T) -> &mut f32),
}

pub(crate) struct Prop<T: 'static> {
    pub(crate) elemento: &'static str,
    /// O nome do atributo e os sinônimos lidos (`w:left`, `w:start`); grava-se o primeiro.
    pub(crate) atributos: &'static [&'static str],
    pub(crate) conv: Conv<T>,
    /// Atributos fixos gravados antes do valor (o `w:val="clear" w:color="auto"` do `w:shd`).
    pub(crate) fixos: &'static str,
    /// Gravado também com este nome, sem ler dele (`w:sz` → `w:szCs`).
    pub(crate) gemeo: Option<&'static str>,
    /// Grava mesmo com o valor zero quando o elemento sai (o `w:left`/`w:right` do `w:ind`).
    pub(crate) sempre: bool,
}

const fn p<T>(elemento: &'static str, atributos: &'static [&'static str], conv: Conv<T>) -> Prop<T> {
    Prop { elemento, atributos, conv, fixos: "", gemeo: None, sempre: false }
}

const SOMBRA: &str = " w:val=\"clear\" w:color=\"auto\"";
const ALINHAMENTOS: [Alinhamento; 4] = [Alinhamento::Esquerda, Alinhamento::Centro, Alinhamento::Direita, Alinhamento::Justificado];

/// `w:rPr`.
pub(crate) const CARACTERE: &[Prop<Car>] = &[
    p("w:b", &["w:val"], Conv::Bit(|c| &mut c.0, NEGRITO, "")),
    p("w:i", &["w:val"], Conv::Bit(|c| &mut c.0, ITALICO, "")),
    p("w:strike", &["w:val"], Conv::Bit(|c| &mut c.0, TACHADO, "")),
    p("w:color", &["w:val"], Conv::Cor(|c| &mut c.1.cor)),
    Prop { gemeo: Some("w:szCs"), ..p("w:sz", &["w:val"], Conv::Medida(|c| &mut c.1.tamanho, 2.0)) },
    p("w:highlight", &["w:val"], Conv::CorComNome(|c| &mut c.1.realce)),
    p("w:u", &["w:val"], Conv::Bit(|c| &mut c.0, SUBLINHADO, "single")),
    Prop { fixos: SOMBRA, ..p("w:shd", &["w:fill"], Conv::Cor(|c| &mut c.1.realce)) },
    p(
        "w:vertAlign",
        &["w:val"],
        Conv::Opcoes(
            |c, v| {
                if let Some(v) = v {
                    c.1.posicao = v;
                }
                c.1.posicao
            },
            &[("superscript", 1), ("subscript", -1)],
        ),
    ),
];

/// `w:pPr`.
pub(crate) const PARAGRAFO: &[Prop<Atrib>] = &[
    Prop { fixos: SOMBRA, ..p("w:shd", &["w:fill"], Conv::Cor(|a| &mut a.fundo)) },
    p("w:spacing", &["w:before"], Conv::Opcional(|a| &mut a.antes, 20.0)),
    p("w:spacing", &["w:after"], Conv::Opcional(|a| &mut a.depois, 20.0)),
    p("w:spacing", &["w:line"], Conv::Entrelinha(|a| &mut a.entrelinha)),
    Prop { sempre: true, ..p("w:ind", &["w:left", "w:start"], Conv::Medida(|a| &mut a.recuo_esq, 20.0)) },
    Prop { sempre: true, ..p("w:ind", &["w:right", "w:end"], Conv::Medida(|a| &mut a.recuo_dir, 20.0)) },
    p("w:ind", &["w:firstLine", "w:hanging"], Conv::PrimeiraLinha(|a| &mut a.primeira)),
    p(
        "w:jc",
        &["w:val"],
        Conv::Opcoes(
            |a, v| {
                if let Some(v) = v {
                    a.alinhamento = ALINHAMENTOS[usize::try_from(v).unwrap_or(0).min(3)];
                }
                ALINHAMENTOS.iter().position(|x| *x == a.alinhamento).unwrap_or(0) as i8
            },
            &[("center", 1), ("right", 2), ("end", 2), ("both", 3), ("distribute", 3)],
        ),
    ),
];

/// Lê o elemento `nome` para `v`, se ele estiver no mapa (`false` se não está).
pub(crate) fn ler<T>(mapa: &[Prop<T>], nome: &str, e: &BytesStart, v: &mut T) -> bool {
    let mut achou = false;
    for p in mapa.iter().filter(|p| p.elemento == nome) {
        achou = true;
        let num = || p.atributos.iter().find_map(|a| numero(e, a));
        let texto = || atributo(e, p.atributos[0]);
        match &p.conv {
            Conv::Bit(f, bit, _) => {
                if ligado(e) {
                    *f(v) |= bit;
                } else {
                    *f(v) &= !bit;
                }
            }
            Conv::Medida(f, por) => *f(v) = num().map_or(0.0, |x| x / por),
            Conv::Opcional(f, por) => {
                if let Some(x) = num() {
                    *f(v) = Some(x / por);
                }
            }
            Conv::Cor(f) => *f(v) = texto().map_or(0, |x| cor(&x)),
            Conv::CorComNome(f) => *f(v) = texto().map_or(0, |x| cor_do_realce(&x)),
            Conv::Opcoes(f, valores) => {
                let codigo = texto().and_then(|x| valores.iter().find(|(n, _)| *n == x).map(|(_, c)| *c)).unwrap_or(0);
                f(v, Some(codigo));
            }
            Conv::Entrelinha(f) => {
                let auto = matches!(atributo(e, "w:lineRule").as_deref(), None | Some("auto"));
                if let (Some(l), true) = (numero(e, "w:line"), auto) {
                    *f(v) = l / 240.0;
                }
            }
            Conv::PrimeiraLinha(f) => *f(v) = numero(e, "w:firstLine").map(|x| x / 20.0).or_else(|| numero(e, "w:hanging").map(|h| -h / 20.0)).unwrap_or(0.0),
        }
    }
    achou
}

/// O atributo gravado (` w:x="…"`; vazio para o liga/desliga sem valor), ou `None` se o valor é o
/// padrão; com `forcar`, a medida zero sai também.
fn atributo_gravado<T>(p: &Prop<T>, v: &mut T, forcar: bool) -> Option<String> {
    let nome = p.atributos[0];
    let inteiro = |x: f32| (x.round() as i64).to_string();
    let valor = match &p.conv {
        Conv::Bit(f, bit, gravado) => {
            if *f(v) & bit == 0 {
                return None;
            }
            if gravado.is_empty() {
                return Some(String::new());
            }
            (*gravado).to_string()
        }
        Conv::Medida(f, por) => {
            let x = *f(v);
            if x == 0.0 && !forcar {
                return None;
            }
            inteiro(x * por)
        }
        Conv::Opcional(f, por) => inteiro((*f(v))? * por),
        Conv::Cor(f) => {
            let c = *f(v);
            if c == 0 {
                return None;
            }
            format!("{:06X}", c >> 8)
        }
        Conv::CorComNome(_) => return None,
        Conv::Opcoes(f, valores) => {
            let c = f(v, None);
            valores.iter().find(|(_, x)| *x == c && c != 0)?.0.to_string()
        }
        Conv::Entrelinha(f) => {
            let x = *f(v);
            if x <= 0.0 {
                return None;
            }
            return Some(format!(" {nome}=\"{}\" w:lineRule=\"auto\"", inteiro(x * 240.0)));
        }
        Conv::PrimeiraLinha(f) => {
            let x = *f(v);
            return match x {
                x if x < 0.0 => Some(format!(" w:hanging=\"{}\"", inteiro(-x * 20.0))),
                x if x > 0.0 => Some(format!(" w:firstLine=\"{}\"", inteiro(x * 20.0))),
                _ => None,
            };
        }
    };
    Some(format!("{} {nome}=\"{valor}\"", p.fixos))
}

/// Os elementos do mapa com valor diferente do padrão, na ordem do mapa (vazio = nenhum).
pub(crate) fn gravar<T: Copy>(mapa: &[Prop<T>], mut v: T) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < mapa.len() {
        let elemento = mapa[i].elemento;
        let grupo = &mapa[i..i + mapa[i..].iter().take_while(|p| p.elemento == elemento).count()];
        i += grupo.len();
        if grupo.iter().all(|p| atributo_gravado(p, &mut v, false).is_none()) {
            continue;
        }
        let atributos: String = grupo.iter().filter_map(|p| atributo_gravado(p, &mut v, p.sempre)).collect();
        for nome in std::iter::once(elemento).chain(grupo[0].gemeo) {
            s.push_str(&format!("<{nome}{atributos}/>"));
        }
    }
    s
}
