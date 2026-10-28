//! O mapa único das propriedades de caractere (`w:rPr`) e de parágrafo (`w:pPr`) do `.docx`: cada
//! entrada diz o elemento e o atributo do OOXML, o campo do modelo e a conversão. O leitor
//! (`docx.rs`) e o gravador (`docx_gravar.rs`) percorrem as mesmas tabelas: uma propriedade nova é
//! uma linha aqui, e ela já vai e volta. As entradas de um mesmo elemento ficam juntas, e os
//! elementos na ordem que o esquema do Word exige.
//!
//! Fora do mapa, escritos à parte: `w:rStyle` e `w:pStyle` (apontam estilos), a classe da fonte (os
//! bits `SERIFA`/`CODIGO`, tirados do nome que o mapa lê, e o `w:rFonts` da classe quando o trecho
//! não tem nome) e os elementos com filhos: `w:numPr` (aponta numerações), `w:pBdr` (os lados numa
//! máscara de bits, com uma cor, um traço e uma espessura só) e `w:tabs` (a lista de paradas). Estes
//! três ficam no mapa como `Conv::Fora`, só para o gravador os pôr na ordem do esquema.

use featherdocs_core::modelo::{SEM_HIFENIZAR, 
    Alinhamento, Atrib, Familia, Fmt, ANTES_AUTOMATICO, BAIXO_RELEVO, CONTORNO, DEPOIS_AUTOMATICO, DISTRIBUIDO, ESPACO_CONTEXTUAL, ITALICO, LINHAS_JUNTAS, MAIUSCULAS, MANTER_COM_PROXIMO, NEGRITO, OCULTO, QUEBRA_ANTES, RELEVO, SOMBRA as SOMBRA_DA_LETRA, SUBLINHADO, TACHADO, TACHADO_DUPLO, VERSALETE, VIUVAS_LIVRES,
};
use quick_xml::events::BytesStart;

use super::{atributo, cor, cor_do_realce, ligado, numero};

/// O caractere: os bits de `Trecho::estilo` e o `Fmt`.
pub(crate) type Car = (u8, Fmt);

/// Como o valor do atributo vira o campo do modelo, e volta.
pub(crate) enum Conv<T> {
    /// Liga/desliga um bit (`w:val="0"`, `false`, `none`, `off` desliga). Grava este valor no
    /// atributo; vazio = o elemento sem atributo (`<w:b/>`). Num atributo que não é o `w:val`
    /// (`w:beforeAutospacing`), só o atributo presente mexe. Desligado à vista (num estilo) grava "0".
    Bit(fn(&mut T) -> &mut u8, u8, &'static str),
    /// O bit ligado = o elemento desligado (`<w:widowControl w:val="0"/>`).
    Desligado(fn(&mut T) -> &mut u8, u8),
    /// Medida em pontos; o arquivo guarda esta quantidade por ponto (vigésimos: 20, meios-pontos: 2).
    /// Ausente = 0.
    Medida(fn(&mut T) -> &mut f32, f32),
    /// Como `Medida`, opcional: ausente não mexe (`None` = a do tipo de bloco).
    Opcional(fn(&mut T) -> &mut Option<f32>, f32),
    /// `RRGGBB` ↔ `0xRRGGBBFF`; `auto` e ausente = 0.
    Cor(fn(&mut T) -> &mut u32),
    /// As cores com nome do `w:highlight`: a cor e o código do nome (posição em `REALCES` + 1). Só
    /// grava se a cor ainda é a do nome (senão o `w:shd` grava a cor).
    CorComNome(fn(&mut T) -> &mut u32, fn(&mut T) -> &mut u8),
    /// Um valor de uma lista: a função lê o código (e o troca, com `Some`). O código 0 é o padrão,
    /// que não se grava; o primeiro nome de cada código é o gravado, os outros são sinônimos.
    Opcoes(fn(&mut T, Option<i8>) -> i8, &'static [(&'static str, i8)]),
    /// Entrelinha: `w:line` em 240 avos com `w:lineRule="auto"` (ou sem ele) na múltipla, em vigésimos
    /// de ponto com `atLeast` na mínima e com `exact` na exata (a lida zera as outras); ausente não mexe.
    Entrelinha(fn(&mut T) -> [&mut f32; 3]),
    /// Letra capitular (`w:framePr w:dropCap`): as linhas, mais 128 se é na margem.
    Capitular(fn(&mut T) -> &mut u8),
    /// Um elemento com filhos, lido e gravado à parte (`w:numPr`, `w:pBdr`, `w:tabs`): o gravador põe
    /// o XML que recebe dele neste lugar.
    Fora,
    /// Recuo da primeira linha em pontos: `w:firstLine` positivo, `w:hanging` negativo.
    PrimeiraLinha(fn(&mut T) -> &mut f32),
    /// O nome da fonte: `w:ascii`/`w:hAnsi` (grava também o `w:cs`), ou a fonte do tema
    /// `w:asciiTheme`/`w:hAnsiTheme` (`+majorHAnsi`), que ganha do nome. Sem nenhum deles, não mexe
    /// (o `w:rFonts` só com `w:eastAsia` fica como estava).
    Familia(fn(&mut T) -> &mut Familia),
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
    /// Não grava quando isto é verdade (o `w:shd` do realce que sai como `w:highlight`).
    pub(crate) exceto: Option<fn(&mut T) -> bool>,
}

const fn p<T>(elemento: &'static str, atributos: &'static [&'static str], conv: Conv<T>) -> Prop<T> {
    Prop { elemento, atributos, conv, fixos: "", gemeo: None, sempre: false, exceto: None }
}

const fn fora<T>(elemento: &'static str) -> Prop<T> {
    p(elemento, &[], Conv::Fora)
}

/// As cores com nome do `w:highlight` (o código é a posição + 1).
pub(crate) const REALCES: [&str; 16] = ["yellow", "green", "cyan", "magenta", "blue", "red", "darkBlue", "darkCyan", "darkGreen", "darkMagenta", "darkRed", "darkYellow", "darkGray", "lightGray", "black", "white"];

/// O realce do trecho sai como `w:highlight` (o nome guardado ainda é o da cor).
fn realce_com_nome(c: &mut Car) -> bool {
    let n = usize::from(c.1.mais.marca_texto);
    n > 0 && REALCES.get(n - 1).is_some_and(|nome| cor_do_realce(nome) == c.1.realce)
}

/// `w:u`: "none" (0) e os tipos de `TIPOS_DE_SUBLINHADO` (posição + 1).
const SUBLINHADOS: &[(&str, i8)] = &[
    ("none", 0), ("single", 1), ("words", 2), ("double", 3), ("thick", 4), ("dotted", 5), ("dottedHeavy", 6), ("dash", 7), ("dashedHeavy", 8), ("dashLong", 9), ("dashLongHeavy", 10), ("dotDash", 11), ("dashDotHeavy", 12), ("dotDotDash", 13), ("dashDotDotHeavy", 14), ("wave", 15), ("wavyHeavy", 16), ("wavyDouble", 17),
];

/// `w:themeColor`.
const CORES_DO_TEMA: &[(&str, i8)] = &[
    ("dark1", 1), ("light1", 2), ("dark2", 3), ("light2", 4), ("accent1", 5), ("accent2", 6), ("accent3", 7), ("accent4", 8), ("accent5", 9), ("accent6", 10), ("hyperlink", 11), ("followedHyperlink", 12), ("background1", 13), ("text1", 14), ("background2", 15), ("text2", 16),
];

/// O traço das bordas do parágrafo (`w:val` de `w:pBdr`; o código é a posição).
pub(crate) const TRACOS_DE_BORDA: [&str; 12] = ["single", "double", "dotted", "dashed", "thick", "dotDash", "dotDotDash", "triple", "wave", "doubleWave", "threeDEmboss", "threeDEngrave"];

/// O alinhamento e o preenchimento das paradas de tabulação (`w:tab`; o código é a posição).
pub(crate) const ALINHAMENTOS_DE_TABULACAO: [&str; 7] = ["left", "center", "right", "decimal", "bar", "clear", "num"];
pub(crate) const PREENCHIMENTOS_DE_TABULACAO: [&str; 6] = ["none", "dot", "hyphen", "underscore", "heavy", "middleDot"];

const fn efeito(elemento: &'static str, bit: u8) -> Prop<Car> {
    p(elemento, &["w:val"], Conv::Bit(|c| &mut c.1.mais.efeitos, bit, ""))
}

const fn fluxo(elemento: &'static str, bit: u8) -> Prop<Atrib> {
    p(elemento, &["w:val"], Conv::Bit(|a| &mut a.mais.fluxo, bit, ""))
}

const SOMBRA: &str = " w:val=\"clear\" w:color=\"auto\"";
const ALINHAMENTOS: [Alinhamento; 4] = [Alinhamento::Esquerda, Alinhamento::Centro, Alinhamento::Direita, Alinhamento::Justificado];

/// `w:rPr`.
pub(crate) const CARACTERE: &[Prop<Car>] = &[
        p("w:rFonts", &["w:ascii"], Conv::Familia(|c| &mut c.1.familia)),
        p("w:b", &["w:val"], Conv::Bit(|c| &mut c.0, NEGRITO, "")),
        p("w:i", &["w:val"], Conv::Bit(|c| &mut c.0, ITALICO, "")),
        efeito("w:caps", MAIUSCULAS),
        efeito("w:smallCaps", VERSALETE),
        p("w:strike", &["w:val"], Conv::Bit(|c| &mut c.0, TACHADO, "")),
        efeito("w:dstrike", TACHADO_DUPLO),
        efeito("w:outline", CONTORNO),
        efeito("w:shadow", SOMBRA_DA_LETRA),
        efeito("w:emboss", RELEVO),
        efeito("w:imprint", BAIXO_RELEVO),
        efeito("w:vanish", OCULTO),
        p("w:color", &["w:val"], Conv::Cor(|c| &mut c.1.cor)),
        p(
            "w:color",
            &["w:themeColor"],
            Conv::Opcoes(
                |c, v| {
                    if let Some(v) = v {
                        c.1.mais.cor_tema = v as u8;
                    }
                    c.1.mais.cor_tema as i8
                },
                CORES_DO_TEMA,
            ),
        ),
        p("w:spacing", &["w:val"], Conv::Medida(|c| &mut c.1.mais.espacamento, 20.0)),
        p("w:w", &["w:val"], Conv::Medida(|c| &mut c.1.mais.escala, 1.0)),
        p("w:kern", &["w:val"], Conv::Medida(|c| &mut c.1.mais.kerning, 2.0)),
        p("w:position", &["w:val"], Conv::Medida(|c| &mut c.1.mais.elevacao, 2.0)),
        Prop { gemeo: Some("w:szCs"), ..p("w:sz", &["w:val"], Conv::Medida(|c| &mut c.1.tamanho, 2.0)) },
        p("w:highlight", &["w:val"], Conv::CorComNome(|c| &mut c.1.realce, |c| &mut c.1.mais.marca_texto)),
        p(
            "w:u",
            &["w:val"],
            Conv::Opcoes(
                |c, v| {
                    if let Some(v) = v {
                        if v > 0 {
                            c.0 |= SUBLINHADO;
                        } else {
                            c.0 &= !SUBLINHADO;
                        }
                        c.1.mais.sublinhado = v.max(1) as u8 - 1;
                    }
                    if c.0 & SUBLINHADO == 0 { 0 } else { c.1.mais.sublinhado as i8 + 1 }
                },
                SUBLINHADOS,
            ),
        ),
        p("w:u", &["w:color"], Conv::Cor(|c| &mut c.1.mais.cor_sublinhado)),
        Prop { fixos: SOMBRA, exceto: Some(realce_com_nome), ..p("w:shd", &["w:fill"], Conv::Cor(|c| &mut c.1.realce)) },
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
        fluxo("w:keepNext", MANTER_COM_PROXIMO),
        fluxo("w:keepLines", LINHAS_JUNTAS),
        fluxo("w:pageBreakBefore", QUEBRA_ANTES),
        p("w:framePr", &["w:dropCap"], Conv::Capitular(|a| &mut a.mais.capitular)),
        p("w:widowControl", &["w:val"], Conv::Desligado(|a| &mut a.mais.fluxo, VIUVAS_LIVRES)),
        fora("w:numPr"),
        fora("w:pBdr"),
        Prop { fixos: SOMBRA, ..p("w:shd", &["w:fill"], Conv::Cor(|a| &mut a.fundo)) },
        fora("w:tabs"),
        p("w:suppressAutoHyphens", &["w:val"], Conv::Bit(|a| &mut a.mais.mais_fluxo, SEM_HIFENIZAR, "")),
        p("w:spacing", &["w:before"], Conv::Opcional(|a| &mut a.antes, 20.0)),
        p("w:spacing", &["w:beforeAutospacing"], Conv::Bit(|a| &mut a.mais.fluxo, ANTES_AUTOMATICO, "1")),
        p("w:spacing", &["w:after"], Conv::Opcional(|a| &mut a.depois, 20.0)),
        p("w:spacing", &["w:afterAutospacing"], Conv::Bit(|a| &mut a.mais.fluxo, DEPOIS_AUTOMATICO, "1")),
        p("w:spacing", &["w:line"], Conv::Entrelinha(|a| [&mut a.entrelinha, &mut a.mais.entrelinha_minima, &mut a.mais.entrelinha_exata])),
        Prop { sempre: true, ..p("w:ind", &["w:left", "w:start"], Conv::Medida(|a| &mut a.recuo_esq, 20.0)) },
        Prop { sempre: true, ..p("w:ind", &["w:right", "w:end"], Conv::Medida(|a| &mut a.recuo_dir, 20.0)) },
        p("w:ind", &["w:firstLine", "w:hanging"], Conv::PrimeiraLinha(|a| &mut a.primeira)),
        fluxo("w:contextualSpacing", ESPACO_CONTEXTUAL),
        p(
            "w:jc",
            &["w:val"],
            Conv::Opcoes(
                |a, v| {
                    if let Some(v) = v {
                        a.alinhamento = ALINHAMENTOS[usize::try_from(v).unwrap_or(0).min(3)];
                        a.mais.fluxo = if v == 4 { a.mais.fluxo | DISTRIBUIDO } else { a.mais.fluxo & !DISTRIBUIDO };
                    }
                    match ALINHAMENTOS.iter().position(|x| *x == a.alinhamento).unwrap_or(0) as i8 {
                        3 if a.mais.fluxo & DISTRIBUIDO != 0 => 4,
                        c => c,
                    }
                },
                &[("left", 0), ("start", 0), ("center", 1), ("right", 2), ("end", 2), ("both", 3), ("distribute", 4)],
            ),
        ),
        p(
            "w:outlineLvl",
            &["w:val"],
            Conv::Opcoes(
                |a, v| {
                    if let Some(v) = v {
                        a.mais.nivel_topico = v as u8;
                    }
                    a.mais.nivel_topico as i8
                },
                &[("0", 1), ("1", 2), ("2", 3), ("3", 4), ("4", 5), ("5", 6), ("6", 7), ("7", 8), ("8", 9)],
            ),
        ),
];

/// Lê o elemento `nome` para `v`, se ele estiver no mapa (`false` se não está).
pub(crate) fn ler<T>(mapa: &[Prop<T>], nome: &str, e: &BytesStart, v: &mut T) -> bool {
    let mut achou = false;
    for p in mapa.iter().filter(|p| p.elemento == nome) {
        let num = || p.atributos.iter().find_map(|a| numero(e, a));
        let texto = || atributo(e, p.atributos[0]);
        achou = true;
        match &p.conv {
            Conv::Bit(f, bit, _) => {
                let liga = if p.atributos[0] == "w:val" { Some(ligado(e)) } else { texto().map(|x| !matches!(x.as_str(), "0" | "false" | "off")) };
                match liga {
                    Some(true) => *f(v) |= bit,
                    Some(false) => *f(v) &= !bit,
                    None => {}
                }
            }
            Conv::Desligado(f, bit) => {
                if ligado(e) {
                    *f(v) &= !bit;
                } else {
                    *f(v) |= bit;
                }
            }
            Conv::Medida(f, por) => *f(v) = num().map_or(0.0, |x| x / por),
            Conv::Opcional(f, por) => {
                if let Some(x) = num() {
                    *f(v) = Some(x / por);
                }
            }
            Conv::Cor(f) => *f(v) = texto().map_or(0, |x| cor(&x)),
            Conv::CorComNome(f, nome) => {
                let x = texto().unwrap_or_default();
                *f(v) = cor_do_realce(&x);
                *nome(v) = REALCES.iter().position(|n| *n == x).map_or(0, |i| i as u8 + 1);
            }
            Conv::Opcoes(f, valores) => {
                let codigo = texto().and_then(|x| valores.iter().find(|(n, _)| *n == x).map(|(_, c)| *c)).unwrap_or(0);
                f(v, Some(codigo));
            }
            Conv::Entrelinha(f) => {
                let regra = match atributo(e, "w:lineRule").as_deref() {
                    None | Some("auto") => 0,
                    Some("atLeast") => 1,
                    _ => 2,
                };
                if let Some(l) = numero(e, "w:line") {
                    for (k, x) in f(v).into_iter().enumerate() {
                        *x = if k != regra { 0.0 } else if regra == 0 { l / 240.0 } else { l / 20.0 };
                    }
                }
            }
            Conv::Capitular(f) => {
                let linhas = numero(e, "w:lines").map_or(3, |n| n.clamp(1.0, 10.0) as u8);
                *f(v) = match texto().as_deref() {
                    Some("drop") => linhas,
                    Some("margin") => linhas | 128,
                    _ => 0,
                };
            }
            Conv::PrimeiraLinha(f) => *f(v) = numero(e, "w:firstLine").map(|x| x / 20.0).or_else(|| numero(e, "w:hanging").map(|h| -h / 20.0)).unwrap_or(0.0),
            Conv::Familia(f) => {
                let tema = atributo(e, "w:hAnsiTheme").or_else(|| atributo(e, "w:asciiTheme")).map(|t| format!("+{t}"));
                if let Some(nome) = tema.or_else(|| atributo(e, "w:ascii")).or_else(|| atributo(e, "w:hAnsi")).filter(|n| !n.trim().is_empty()) {
                    *f(v) = Familia::de(&nome);
                }
            }
            Conv::Fora => achou = false,
        }
    }
    achou
}

/// O atributo gravado (` w:x="…"`; vazio para o liga/desliga sem valor), ou `None` se o valor é o
/// padrão; com `forcar`, a medida zero sai também. `desl` = o que se grava desligado à vista (o
/// estilo que desfaz o do pai): os bits ligados nele saem com "0", e a opção dele com o código 0.
fn atributo_gravado<T>(p: &Prop<T>, v: &mut T, desl: &mut T, forcar: bool) -> Option<String> {
    let nome = p.atributos.first().copied().unwrap_or("");
    let inteiro = |x: f32| (x.round() as i64).to_string();
    if p.exceto.is_some_and(|f| f(v)) {
        return None;
    }
    let valor = match &p.conv {
        Conv::Bit(f, bit, gravado) => {
            if *f(v) & bit == 0 {
                if *f(desl) & bit == 0 {
                    return None;
                }
                "0".to_string()
            } else if gravado.is_empty() {
                return Some(String::new());
            } else {
                (*gravado).to_string()
            }
        }
        Conv::Desligado(f, bit) => {
            if *f(v) & bit != 0 {
                "0".to_string()
            } else if *f(desl) & bit != 0 {
                return Some(String::new());
            } else {
                return None;
            }
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
        Conv::CorComNome(f, n) => {
            let (c, i) = (*f(v), usize::from(*n(v)));
            REALCES.get(i.checked_sub(1)?).filter(|nome| cor_do_realce(nome) == c)?.to_string()
        }
        Conv::Opcoes(f, valores) => {
            let c = f(v, None);
            let desligada = c == 0 && f(desl, None) != 0;
            valores.iter().find(|(_, x)| *x == c && (c != 0 || forcar || desligada))?.0.to_string()
        }
        Conv::Entrelinha(f) => {
            let [multipla, minima, exata] = f(v).map(|x| *x);
            return match () {
                () if exata > 0.0 => Some(format!(" {nome}=\"{}\" w:lineRule=\"exact\"", inteiro(exata * 20.0))),
                () if minima > 0.0 => Some(format!(" {nome}=\"{}\" w:lineRule=\"atLeast\"", inteiro(minima * 20.0))),
                () if multipla > 0.0 => Some(format!(" {nome}=\"{}\" w:lineRule=\"auto\"", inteiro(multipla * 240.0))),
                () => None,
            };
        }
        Conv::Capitular(f) => {
            let c = *f(v);
            if c == 0 {
                return None;
            }
            let tipo = if c & 128 != 0 { "margin" } else { "drop" };
            return Some(format!(" w:dropCap=\"{tipo}\" w:lines=\"{}\" w:wrap=\"around\" w:vAnchor=\"text\" w:hAnchor=\"text\"", c & 127));
        }
        Conv::Familia(f) => {
            let familia = *f(v);
            if familia.vazia() {
                return None;
            }
            if let Some(t) = familia.do_tema() {
                return Some(format!(" w:asciiTheme=\"{t}\" w:hAnsiTheme=\"{t}\""));
            }
            let n = familia.nome().replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
            return Some(format!(" w:ascii=\"{n}\" w:hAnsi=\"{n}\" w:cs=\"{n}\""));
        }
        Conv::PrimeiraLinha(f) => {
            let x = *f(v);
            return match x {
                x if x < 0.0 => Some(format!(" w:hanging=\"{}\"", inteiro(-x * 20.0))),
                x if x > 0.0 => Some(format!(" w:firstLine=\"{}\"", inteiro(x * 20.0))),
                _ => None,
            };
        }
        Conv::Fora => return None,
    };
    Some(format!("{} {nome}=\"{valor}\"", p.fixos))
}

/// Os elementos do mapa com valor diferente do padrão, na ordem do mapa (vazio = nenhum); os de
/// `forcados` também com o valor padrão (o `w:ind` zero, o `w:jc` à esquerda que desfaz o do estilo),
/// o que `desl` desliga à vista (ver `atributo_gravado`) e o XML dos elementos `Conv::Fora`
/// (`(elemento, xml)`), cada um no lugar dele.
pub(crate) fn gravar_tudo<T: Copy>(mapa: &[Prop<T>], mut v: T, mut desl: T, forcados: &[&str], fora: &[(&str, &str)]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < mapa.len() {
        let elemento = mapa[i].elemento;
        let grupo = &mapa[i..i + mapa[i..].iter().take_while(|p| p.elemento == elemento).count()];
        i += grupo.len();
        if matches!(grupo[0].conv, Conv::Fora) {
            s.extend(fora.iter().filter(|(e, _)| *e == elemento).map(|(_, x)| *x));
            continue;
        }
        let forcado = forcados.contains(&elemento);
        if !forcado && grupo.iter().all(|p| atributo_gravado(p, &mut v, &mut desl, false).is_none()) {
            continue;
        }
        let atributos: String = grupo.iter().filter_map(|p| atributo_gravado(p, &mut v, &mut desl, p.sempre || forcado)).collect();
        for nome in std::iter::once(elemento).chain(grupo[0].gemeo) {
            s.push_str(&format!("<{nome}{atributos}/>"));
        }
    }
    s
}
