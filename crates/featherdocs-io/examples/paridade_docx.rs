//! Mede a paridade do `.docx` (ida e volta `.fdoz` ↔ `.docx`) e gera `featherdocs/docs/PARIDADE-DOCX.md`.
//!
//! Cada item é um `.docx` mínimo montado aqui (XML à mão num ZIP) com o texto `ZQX` no lugar do
//! recurso. Três medidas, todas tiradas do modelo (`featherdocs-core/src/modelo.rs`), não do código:
//!   **Lê**    — `docx::ler` põe o efeito no modelo? (a função `le` do item olha o modelo);
//!   **Grava** — o modelo (o lido, ou um montado direto quando o leitor não preenche o campo) vai
//!               para `docx::gravar` e o elemento aparece no XML de saída (padrões `grava`);
//!   **Volta** — `.docx` → modelo → `.fdoz` → modelo → `.docx` → modelo dá o mesmo resultado de `le`?
//! Marcas: `[x]` completo · `[~]` parcial · `[ ]` falta.
//! Rodar (a partir de `featherdocs/crates/featherdocs-io`):
//! `cargo run --example paridade_docx -- --saida ../../docs/PARIDADE-DOCX.md` (sem `--saida`
//! imprime na tela; `--detalhe` põe na tabela o que cada medida viu).
//! O teste `tests/paridade_docx.rs` usa este arquivo como módulo (`#[path]`): a catraca roda a
//! mesma medição e falha se algum item piorar em relação a `dados/paridade_docx_base.txt`.

#![allow(clippy::too_many_lines, clippy::cognitive_complexity)]

use featherdocs_core::inline::texto_puro;
use featherdocs_core::modelo::*;
use featherdocs_io::{base64, docx, fdoz};
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// O texto que marca o recurso medido.
const Z: &str = "ZQX";

/// `(marca, o que se viu)`; o "o que se viu" é o que a volta compara.
pub(crate) type Medida = (char, String);

pub(crate) struct Item {
    pub(crate) secao: &'static str,
    pub(crate) nome: &'static str,
    corpo: String,
    partes: Vec<(&'static str, String)>,
    le: fn(&Documento) -> Medida,
    /// Padrões que o XML gravado precisa ter (`parte>padrão`; sem `parte>` = `document.xml`;
/// `!` na frente = não pode ter).
    grava: Vec<&'static str>,
    /// Se `grava` falha mas estes estão lá: `[~]` (grava outra coisa parecida).
    parcial: Vec<&'static str>,
    /// O modelo montado direto, para medir o gravador quando o leitor não preenche o campo.
    montar: Option<fn() -> Documento>,
}

impl Item {
    fn partes(mut self, p: &[(&'static str, String)]) -> Self {
        self.partes.extend(p.iter().cloned());
        self
    }
    fn parcial(mut self, p: &[&'static str]) -> Self {
        self.parcial = p.to_vec();
        self
    }
    fn montar(mut self, f: fn() -> Documento) -> Self {
        self.montar = Some(f);
        self
    }
}

fn it(secao: &'static str, nome: &'static str, corpo: impl Into<String>, le: fn(&Documento) -> Medida, grava: &[&'static str]) -> Item {
    Item { secao, nome, corpo: corpo.into(), partes: Vec::new(), le, grava: grava.to_vec(), parcial: Vec::new(), montar: None }
}

// ---------------------------------------------------------------------------
// O .docx mínimo
// ---------------------------------------------------------------------------

const NS: &str = concat!(
    "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" ",
    "xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" ",
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" ",
    "xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\" ",
    "xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" ",
    "xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" ",
    "xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" ",
    "xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\" ",
    "xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" ",
    "xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\""
);

/// Um PNG de 1×1.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn estilos_padrao() -> String {
    format!(
        "<w:styles {NS}>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Ttulo1\"><w:name w:val=\"heading 1\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"0\"/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Ttulo2\"><w:name w:val=\"heading 2\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"1\"/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Ttulo\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Citao\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Legenda\"><w:name w:val=\"caption\"/><w:basedOn w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Sumrio1\"><w:name w:val=\"toc 1\"/><w:basedOn w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"CabealhodoSumrio\"><w:name w:val=\"TOC Heading\"/><w:basedOn w:val=\"Ttulo1\"/></w:style>\
<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>\
</w:styles>"
    )
}

/// Níveis: `(numFmt, lvlText)` para os níveis 0, 1, 2.
fn abstrata(id: u32, niveis: &[(&str, &str)], extra: &str) -> String {
    let mut s = format!("<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>");
    for (i, (fmt, txt)) in niveis.iter().enumerate() {
        let _ = write!(s, "<w:lvl w:ilvl=\"{i}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{txt}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>", 720 * (i + 1));
    }
    s.push_str(extra);
    s.push_str("</w:abstractNum>");
    s
}

/// numId 1 = decimal (a, i nos níveis 1, 2) · 2 = marcadores · 3 = como o 1, recomeçando ·
/// 4 = seta ➢ · 5 = começa em 5 · 6 = estilo de lista (numStyleLink) · 7 = "Passo %1:" ·
/// 8 = símbolo da Wingdings · 9 = tarefa ☐ · 10 = letras no nível 0.
fn numeracao_padrao() -> String {
    let mut s = format!("<w:numbering {NS}>");
    s.push_str(&abstrata(0, &[("decimal", "%1."), ("lowerLetter", "%2)"), ("lowerRoman", "%3.")], ""));
    s.push_str(&abstrata(1, &[("bullet", "•"), ("bullet", "◦"), ("bullet", "▪")], ""));
    s.push_str(&abstrata(2, &[("bullet", "➢")], ""));
    s.push_str(&abstrata(3, &[("decimal", "%1.")], "").replace("<w:start w:val=\"1\"/>", "<w:start w:val=\"5\"/>"));
    s.push_str("<w:abstractNum w:abstractNumId=\"4\"><w:numStyleLink w:val=\"MinhaLista\"/></w:abstractNum>");
    s.push_str(&abstrata(5, &[("decimal", "Passo %1:")], ""));
    s.push_str(&abstrata(6, &[("bullet", "\u{F0D8}")], "").replace("<w:lvlJc", "<w:rPr><w:rFonts w:ascii=\"Wingdings\" w:hAnsi=\"Wingdings\"/></w:rPr><w:lvlJc"));
    s.push_str(&abstrata(7, &[("bullet", "☐")], ""));
    s.push_str(&abstrata(8, &[("lowerLetter", "%1)")], ""));
    for (num, abs) in [(1, 0), (2, 1), (4, 2), (5, 3), (6, 4), (7, 5), (8, 6), (9, 7), (10, 8)] {
        let _ = write!(s, "<w:num w:numId=\"{num}\"><w:abstractNumId w:val=\"{abs}\"/></w:num>");
    }
    s.push_str("<w:num w:numId=\"3\"><w:abstractNumId w:val=\"0\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/></w:lvlOverride></w:num>");
    s.push_str("</w:numbering>");
    s
}

const RELS: &str = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rIdLink\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"http://exemplo.org/\" TargetMode=\"External\"/>\
<Relationship Id=\"rIdImg\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/img.png\"/>\
<Relationship Id=\"rIdImgWeb\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"http://exemplo.org/foto.png\" TargetMode=\"External\"/>\
<Relationship Id=\"rIdCab\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header1.xml\"/>\
<Relationship Id=\"rIdCab2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header2.xml\"/>\
<Relationship Id=\"rIdCab3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header3.xml\"/>\
<Relationship Id=\"rIdRod\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer\" Target=\"footer1.xml\"/>\
<Relationship Id=\"rIdGraf\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart\" Target=\"charts/chart1.xml\"/>\
<Relationship Id=\"rIdOle\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/package\" Target=\"embeddings/planilha.xlsx\"/>\
</Relationships>";

fn montar_docx(item: &Item) -> Vec<u8> {
    let mut partes: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>".to_vec()),
        ("word/document.xml".into(), format!("<w:document {NS}><w:body>{}</w:body></w:document>", item.corpo).into_bytes()),
        ("word/styles.xml".into(), estilos_padrao().into_bytes()),
        ("word/numbering.xml".into(), numeracao_padrao().into_bytes()),
        ("word/_rels/document.xml.rels".into(), RELS.as_bytes().to_vec()),
        ("word/media/img.png".into(), PNG.to_vec()),
    ];
    for (nome, xml) in &item.partes {
        partes.retain(|(n, _)| n != nome);
        partes.push(((*nome).to_string(), xml.clone().into_bytes()));
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (nome, bytes) in partes {
        zip.start_file(nome, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Todas as partes de texto de um `.docx` gravado: `(nome, xml)`.
fn partes_de(bytes: &[u8]) -> Vec<(String, String)> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut v = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut b = Vec::new();
        let _ = f.read_to_end(&mut b);
        if let Ok(s) = String::from_utf8(b) {
            v.push((f.name().to_string(), s));
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Pedaços de XML
// ---------------------------------------------------------------------------

fn r(rpr: &str, t: &str) -> String {
    format!("<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space=\"preserve\">{t}</w:t></w:r>")
}
fn par(ppr: &str, corridas: &str) -> String {
    format!("<w:p><w:pPr>{ppr}</w:pPr>{corridas}</w:p>")
}
/// Parágrafo com "antes " e o `ZQX` com estas propriedades de caractere.
fn pr(rpr: &str) -> String {
    par("", &(r("", "antes ") + &r(rpr, Z)))
}
/// Parágrafo `ZQX` com estas propriedades de parágrafo.
fn pp(ppr: &str) -> String {
    par(ppr, &r("", Z))
}
fn item(num: u32, ilvl: u8, t: &str) -> String {
    pp(&format!("<w:numPr><w:ilvl w:val=\"{ilvl}\"/><w:numId w:val=\"{num}\"/></w:numPr>")).replace(Z, t)
}
fn tc(tcpr: &str, conteudo: &str) -> String {
    format!("<w:tc><w:tcPr>{tcpr}</w:tcPr>{conteudo}</w:tc>")
}
fn cel(t: &str) -> String {
    tc("", &par("", &r("", t)))
}
fn tabela(tblpr: &str, grade: &str, linhas: &[String]) -> String {
    format!("<w:tbl><w:tblPr>{tblpr}</w:tblPr><w:tblGrid>{grade}</w:tblGrid>{}</w:tbl>", linhas.concat())
}
fn tr(trpr: &str, celulas: &[String]) -> String {
    format!("<w:tr><w:trPr>{trpr}</w:trPr>{}</w:tr>", celulas.concat())
}
/// Tabela 2×2 com `ZQX` na primeira célula da segunda linha.
fn tabela2(tblpr: &str, trpr: &str, tcpr: &str) -> String {
    tabela(tblpr, "<w:gridCol w:w=\"4000\"/><w:gridCol w:w=\"4000\"/>", &[tr(trpr, &[cel("A"), cel("B")]), tr("", &[tc(tcpr, &par("", &r("", Z))), cel("2")])])
}
fn desenho(ancora: bool, extra_doc: &str, blip_extra: &str, xfrm: &str) -> String {
    let (abre, fecha) = if ancora {
        ("<wp:anchor distT=\"0\" distB=\"0\" distL=\"114300\" distR=\"114300\" simplePos=\"0\" relativeHeight=\"1\" behindDoc=\"0\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"column\"><wp:posOffset>1270000</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"paragraph\"><wp:posOffset>254000</wp:posOffset></wp:positionV>", "</wp:anchor>")
    } else {
        ("<wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">", "</wp:inline>")
    };
    let fim_ext = if ancora { "<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:wrapSquare wrapText=\"bothSides\"/>" } else { "" };
    format!(
        "<w:drawing>{abre}<wp:extent cx=\"2286000\" cy=\"1143000\"/>{fim_ext}<wp:docPr id=\"1\" name=\"Imagem 1\"{extra_doc}/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"1\" name=\"img.png\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"rIdImg\"/>{blip_extra}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm{xfrm}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"2286000\" cy=\"1143000\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>{fecha}</w:drawing>"
    )
}
fn img_par(d: &str) -> String {
    format!("<w:p><w:r>{d}</w:r></w:p>")
}
fn campo_simples(instr: &str, resultado: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">antes </w:t></w:r><w:fldSimple w:instr=\" {instr} \"><w:r><w:t>{resultado}</w:t></w:r></w:fldSimple></w:p>")
}
fn campo_complexo(instr: &str, resultado: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">antes </w:t></w:r><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>{resultado}</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>")
}
fn sect(dentro: &str) -> String {
    format!("<w:sectPr>{dentro}<w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr>")
}
fn faixa(raiz: &str, corpo: &str) -> String {
    format!("<w:{raiz} {NS}>{corpo}</w:{raiz}>")
}
fn cab(corpo: &str) -> String {
    faixa("hdr", corpo)
}
fn estilos_com(extra: &str) -> String {
    estilos_padrao().replace("</w:styles>", &format!("{extra}</w:styles>"))
}
/// Um `theme1.xml` com só as fontes (o leitor só olha o `a:fontScheme`).
fn tema_com_fontes(titulos: &str, texto: &str) -> String {
    format!("<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Tema\"><a:themeElements><a:fontScheme name=\"Tema\"><a:majorFont><a:latin typeface=\"{titulos}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"{texto}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>")
}
fn settings(dentro: &str) -> String {
    format!("<w:settings {NS}>{dentro}</w:settings>")
}

// ---------------------------------------------------------------------------
// Olhar o modelo
// ---------------------------------------------------------------------------

fn x(s: impl Into<String>) -> Medida {
    ('x', s.into())
}
fn p(s: impl Into<String>) -> Medida {
    ('~', s.into())
}
fn f(s: impl Into<String>) -> Medida {
    (' ', s.into())
}
fn se(c: bool, s: impl Into<String>) -> Medida {
    if c {
        x(s)
    } else {
        f(s)
    }
}
fn sumiu() -> Medida {
    f("ZQX sumiu do texto")
}

/// Todos os trechos do corpo (texto, células, legendas).
fn trechos(d: &Documento) -> Vec<&Trecho> {
    d.blocos.iter().flat_map(|b| b.conteudo.textos()).flatten().collect()
}
/// O primeiro trecho com `ZQX`.
fn alvo(d: &Documento) -> Option<&Trecho> {
    trechos(d).into_iter().find(|t| t.texto.contains(Z))
}
fn com_alvo(d: &Documento, f: impl Fn(&Trecho) -> Medida) -> Medida {
    alvo(d).map_or_else(sumiu, f)
}
/// O bloco que tem `ZQX`.
fn bloco(d: &Documento) -> Option<&Bloco> {
    d.blocos.iter().find(|b| b.conteudo.textos().iter().any(|t| texto_puro(t).contains(Z)) || matches!(&b.conteudo, Conteudo::Codigo { codigo, .. } if codigo.contains(Z)))
}
fn com_bloco(d: &Documento, f: impl Fn(&Bloco) -> Medida) -> Medida {
    bloco(d).map_or_else(sumiu, f)
}
fn quantos_z(d: &Documento) -> usize {
    trechos(d).iter().map(|t| t.texto.matches(Z).count()).sum()
}
fn tipo(b: &Bloco) -> String {
    featherdocs_core::edicao::nome_do_tipo(&b.conteudo)
}
fn tabela_de(d: &Documento) -> Option<(&Bloco, bool, &Vec<Vec<Celula>>)> {
    d.blocos.iter().find_map(|b| if let Conteudo::Tabela { cabecalho, linhas } = &b.conteudo { Some((b, *cabecalho, linhas)) } else { None })
}
fn imagens(d: &Documento) -> Vec<&Bloco> {
    d.blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Imagem { .. })).collect()
}
fn faixas_com_z(f: &Faixas) -> bool {
    [&f.esquerda, &f.centro, &f.direita].iter().any(|t| t.contains(Z))
}
fn marcas(d: &Documento) -> Vec<&Marca> {
    trechos(d).into_iter().filter_map(|t| t.marca.as_deref()).collect()
}
fn campo(d: &Documento) -> Option<String> {
    marcas(d).into_iter().find_map(|m| if let Marca::Campo { campo } = m { Some(campo.clone()) } else { None })
}
fn notas(d: &Documento) -> Vec<(bool, &Vec<Trecho>)> {
    marcas(d).into_iter().filter_map(|m| if let Marca::Nota { fim, texto } = m { Some((*fim, texto)) } else { None }).collect()
}
fn sumario(d: &Documento) -> Option<(u8, bool, u8)> {
    d.blocos.iter().find_map(|b| if let Conteudo::Sumario { niveis, sem_numeros, lista, .. } = &b.conteudo { Some((*niveis, *sem_numeros, *lista)) } else { None })
}
/// O objeto do Word guardado na sacola (o XML original e as partes do pacote, que voltam iguais).
fn na_sacola(d: &Documento) -> Option<Medida> {
    marcas(d).into_iter().find_map(|m| match m {
        Marca::ObjetoWord { resumo, partes, .. } => Some(p(format!("guardado na sacola ({resumo}; partes: {})", partes.iter().map(|p| p.caminho.as_str()).collect::<Vec<_>>().join(", ")))),
        _ => None,
    })
}
/// O controle de conteúdo guardado na sacola: o `w:sdtPr` volta igual, o texto de dentro é texto.
fn controle_na_sacola(d: &Documento, tem: &str) -> Option<Medida> {
    marcas(d).into_iter().find_map(|m| match m {
        Marca::Controle { xml, fim: false, .. } if xml.contains(tem) => Some(p(format!("guardado na sacola (o {tem} volta igual; o texto fica)"))),
        _ => None,
    })
}
fn texto_todo(d: &Documento) -> String {
    trechos(d).iter().map(|t| t.texto.as_str()).collect()
}
fn mm(pt: f32) -> String {
    format!("{:.1}", pt)
}

// ---------------------------------------------------------------------------
// Os itens
// ---------------------------------------------------------------------------

pub(crate) fn itens() -> Vec<Item> {
    let mut v = Vec::new();
    let mut s = "1. Caractere";
    v.push(it(s, "Negrito", pr("<w:b/>"), |d| com_alvo(d, |t| se(t.estilo & NEGRITO != 0, "negrito")), &["<w:b/>"]));
    v.push(it(s, "Itálico", pr("<w:i/>"), |d| com_alvo(d, |t| se(t.estilo & ITALICO != 0, "itálico")), &["<w:i/>"]));
    v.push(it(s, "Sublinhado simples", pr("<w:u w:val=\"single\"/>"), |d| com_alvo(d, |t| se(t.estilo & SUBLINHADO != 0, "sublinhado")), &["<w:u w:val=\"single\""]));
    v.push(it(s, "Sublinhado duplo", pr("<w:u w:val=\"double\"/>"), |d| com_alvo(d, |t| if t.estilo & SUBLINHADO != 0 { p("vira sublinhado simples") } else { f("sem sublinhado") }), &["<w:u w:val=\"double\""]).parcial(&["<w:u "]));
    v.push(it(s, "Sublinhado ondulado/pontilhado (outros tipos)", pr("<w:u w:val=\"wave\"/>"), |d| com_alvo(d, |t| if t.estilo & SUBLINHADO != 0 { p("vira sublinhado simples") } else { f("sem sublinhado") }), &["<w:u w:val=\"wave\""]).parcial(&["<w:u "]));
    v.push(it(s, "Cor do sublinhado", pr("<w:u w:val=\"single\" w:color=\"FF0000\"/>"), |d| com_alvo(d, |t| if t.estilo & SUBLINHADO != 0 { p("sublinhado sem a cor própria") } else { f("sem sublinhado") }), &["w:u w:val=\"single\" w:color=\"FF0000\""]).parcial(&["<w:u "]));
    v.push(it(s, "Tachado", pr("<w:strike/>"), |d| com_alvo(d, |t| se(t.estilo & TACHADO != 0, "tachado")), &["<w:strike/>"]));
    v.push(it(s, "Tachado duplo", pr("<w:dstrike/>"), |d| com_alvo(d, |t| if t.estilo & TACHADO != 0 { p("vira tachado simples") } else { f("sem tachado") }), &["<w:dstrike"]).parcial(&["<w:strike"]));
    v.push(it(s, "Fonte (família)", pr("<w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\" w:cs=\"Georgia\"/>"), |d| com_alvo(d, |t| if t.fmt.familia.nome() == "Georgia" && t.estilo & SERIFA != 0 { x("Georgia (classe serifa)") } else if t.estilo & SERIFA != 0 { p("só a classe: serifa (grava Times New Roman)") } else { f("fonte some") }), &["w:ascii=\"Georgia\""]).parcial(&["<w:rFonts"]));
    v.push(it(s, "Fonte sem serifa qualquer (Calibri)", pr("<w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/>"), |d| com_alvo(d, |t| match (t.estilo & (SERIFA | CODIGO) == 0, t.fmt.familia.nome()) { (true, "Calibri") => x("Calibri (classe sem serifa)"), (true, _) => p("vira a sem serifa padrão (Arial)"), _ => f("classe errada") }), &["w:ascii=\"Calibri\""]));
    v.push(it(s, "Fonte monoespaçada (Consolas)", pr("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\"/>"), |d| com_alvo(d, |t| if t.fmt.familia.nome() == "Consolas" && t.estilo & CODIGO != 0 { x("Consolas (classe código)") } else if t.estilo & CODIGO != 0 { p("vira código (Courier New)") } else { f("fonte some") }), &["w:ascii=\"Consolas\""]).parcial(&["<w:rFonts"]));
    v.push(it(s, "Fonte de tema (majorHAnsi)", pr("<w:rFonts w:asciiTheme=\"majorHAnsi\" w:hAnsiTheme=\"majorHAnsi\"/>"), |d| com_alvo(d, |t| match (t.fmt.familia.do_tema(), d.estilos.nome_da_familia(t.fmt.familia)) {
        (Some("majorHAnsi"), n) if n == "Cambria" && t.estilo & SERIFA != 0 => x("fonte de título do tema: Cambria (serifa)"),
        (Some(_), n) => p(format!("fonte do tema sem o nome certo ({n:?})")),
        _ => f("o tema (theme1.xml) não é lido"),
    }), &["w:asciiTheme=\"majorHAnsi\"", "theme>typeface=\"Cambria\""]).partes(&[("word/theme/theme1.xml", tema_com_fontes("Cambria", "Calibri"))]));
    v.push(it(s, "Tamanho", pr("<w:sz w:val=\"28\"/>"), |d| com_alvo(d, |t| se(t.fmt.tamanho == 14.0, format!("{} pt", t.fmt.tamanho))), &["<w:sz w:val=\"28\""]));
    v.push(it(s, "Cor", pr("<w:color w:val=\"FF0000\"/>"), |d| com_alvo(d, |t| se(t.fmt.cor == 0xFF0000FF, format!("{:08X}", t.fmt.cor))), &["<w:color w:val=\"FF0000\""]));
    v.push(it(s, "Cor de tema (accent1)", pr("<w:color w:val=\"4472C4\" w:themeColor=\"accent1\"/>"), |d| com_alvo(d, |t| if t.fmt.cor == 0x4472C4FF { p("fica a cor fixa, sem o vínculo com o tema") } else { f(format!("{:08X}", t.fmt.cor)) }), &["w:themeColor=\"accent1\""]).parcial(&["<w:color w:val=\"4472C4\""]));
    v.push(it(s, "Realce (marca-texto)", pr("<w:highlight w:val=\"yellow\"/>"), |d| com_alvo(d, |t| se(t.fmt.realce == 0xFFFF00FF, format!("{:08X}", t.fmt.realce))), &["<w:highlight w:val=\"yellow\""]).parcial(&["w:fill=\"FFFF00\""]));
    v.push(it(s, "Sombreamento do caractere", pr("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"D9D9D9\"/>"), |d| com_alvo(d, |t| se(t.fmt.realce == 0xD9D9D9FF, format!("{:08X} (no realce)", t.fmt.realce))), &["w:fill=\"D9D9D9\""]));
    v.push(it(s, "Sobrescrito", pr("<w:vertAlign w:val=\"superscript\"/>"), |d| com_alvo(d, |t| se(t.fmt.posicao == 1, format!("posição {}", t.fmt.posicao))), &["w:vertAlign w:val=\"superscript\""]));
    v.push(it(s, "Subscrito", pr("<w:vertAlign w:val=\"subscript\"/>"), |d| com_alvo(d, |t| se(t.fmt.posicao == -1, format!("posição {}", t.fmt.posicao))), &["w:vertAlign w:val=\"subscript\""]));
    v.push(it(s, "Posição elevada/rebaixada (w:position)", pr("<w:position w:val=\"6\"/>"), |d| com_alvo(d, |t| se(t.fmt.posicao != 0, "o modelo não tem deslocamento")), &["<w:position"]));
    v.push(it(s, "Versalete", pr("<w:smallCaps/>"), |d| com_alvo(d, |_| f("o modelo não tem versalete")), &["<w:smallCaps"]));
    v.push(it(s, "Todas em maiúsculas", pr("<w:caps/>"), |d| com_alvo(d, |_| f("o modelo não tem maiúsculas")), &["<w:caps"]));
    v.push(it(s, "Espaçamento entre letras", pr("<w:spacing w:val=\"40\"/>"), |d| com_alvo(d, |_| f("o modelo não tem espaçamento entre letras")), &["<w:spacing w:val=\"40\""]));
    v.push(it(s, "Escala horizontal", pr("<w:w w:val=\"150\"/>"), |d| com_alvo(d, |_| f("o modelo não tem escala")), &["<w:w w:val=\"150\""]));
    v.push(it(s, "Kerning", pr("<w:kern w:val=\"28\"/>"), |d| com_alvo(d, |_| f("o modelo não tem kerning")), &["<w:kern"]));
    v.push(it(s, "Texto oculto", pr("<w:vanish/>"), |d| com_alvo(d, |_| f("o texto oculto aparece")), &["<w:vanish"]));
    v.push(it(s, "Efeitos (contorno, sombra, relevo)", pr("<w:outline/><w:shadow/>"), |d| com_alvo(d, |_| f("o modelo não tem efeitos")), &["<w:outline", "<w:shadow"]));

    s = "2. Parágrafo";
    v.push(it(s, "Alinhamento centralizado", pp("<w:jc w:val=\"center\"/>"), |d| com_bloco(d, |b| se(b.atrib.alinhamento == Alinhamento::Centro, format!("{:?}", b.atrib.alinhamento))), &["<w:jc w:val=\"center\""]));
    v.push(it(s, "Alinhamento à direita", pp("<w:jc w:val=\"right\"/>"), |d| com_bloco(d, |b| se(b.atrib.alinhamento == Alinhamento::Direita, format!("{:?}", b.atrib.alinhamento))), &["<w:jc w:val=\"right\""]));
    v.push(it(s, "Justificado", pp("<w:jc w:val=\"both\"/>"), |d| com_bloco(d, |b| se(b.atrib.alinhamento == Alinhamento::Justificado, format!("{:?}", b.atrib.alinhamento))), &["<w:jc w:val=\"both\""]));
    v.push(it(s, "Justificado distribuído", pp("<w:jc w:val=\"distribute\"/>"), |d| com_bloco(d, |b| if b.atrib.alinhamento == Alinhamento::Justificado { p("vira justificado comum") } else { f(format!("{:?}", b.atrib.alinhamento)) }), &["<w:jc w:val=\"distribute\""]).parcial(&["<w:jc w:val=\"both\""]));
    v.push(it(s, "Recuos esquerdo e direito", pp("<w:ind w:left=\"720\" w:right=\"360\"/>"), |d| com_bloco(d, |b| se(b.atrib.recuo_esq == 36.0 && b.atrib.recuo_dir == 18.0, format!("{} / {}", b.atrib.recuo_esq, b.atrib.recuo_dir))), &["w:left=\"720\" w:right=\"360\""]));
    v.push(it(s, "Recuo da primeira linha", pp("<w:ind w:firstLine=\"709\"/>"), |d| com_bloco(d, |b| se((b.atrib.primeira - 35.45).abs() < 0.01, format!("{}", b.atrib.primeira))), &["w:firstLine=\"709\""]));
    v.push(it(s, "Recuo deslocado (pendente)", pp("<w:ind w:left=\"720\" w:hanging=\"360\"/>"), |d| com_bloco(d, |b| se(b.atrib.recuo_esq == 36.0 && b.atrib.primeira == -18.0, format!("{} / {}", b.atrib.recuo_esq, b.atrib.primeira))), &["w:hanging=\"360\""]));
    v.push(it(s, "Espaço antes e depois", pp("<w:spacing w:before=\"240\" w:after=\"120\"/>"), |d| com_bloco(d, |b| se(b.atrib.antes == Some(12.0) && b.atrib.depois == Some(6.0), format!("{:?} / {:?}", b.atrib.antes, b.atrib.depois))), &["w:before=\"240\" w:after=\"120\""]));
    v.push(it(s, "Espaço automático antes/depois", pp("<w:spacing w:beforeAutospacing=\"1\" w:afterAutospacing=\"1\"/>"), |d| com_bloco(d, |b| se(b.atrib.antes.is_some(), "o automático vira o espaço do tipo de bloco")), &["w:beforeAutospacing"]));
    v.push(it(s, "Entrelinha múltipla (1,5)", pp("<w:spacing w:line=\"360\" w:lineRule=\"auto\"/>"), |d| com_bloco(d, |b| se(b.atrib.entrelinha == 1.5, format!("{}", b.atrib.entrelinha))), &["w:line=\"360\" w:lineRule=\"auto\""]));
    v.push(it(s, "Entrelinha exata", pp("<w:spacing w:line=\"300\" w:lineRule=\"exact\"/>"), |d| com_bloco(d, |b| se(b.atrib.entrelinha > 0.0, format!("o modelo só tem entrelinha múltipla (ficou {})", b.atrib.entrelinha))), &["w:lineRule=\"exact\""]));
    v.push(it(s, "Entrelinha \"pelo menos\"", pp("<w:spacing w:line=\"300\" w:lineRule=\"atLeast\"/>"), |d| com_bloco(d, |b| se(b.atrib.entrelinha > 0.0, format!("o modelo só tem entrelinha múltipla (ficou {})", b.atrib.entrelinha))), &["w:lineRule=\"atLeast\""]));
    v.push(it(s, "Não somar espaço entre parágrafos do mesmo estilo", pp("<w:contextualSpacing/>"), |d| com_bloco(d, |_| f("o modelo não tem")), &["<w:contextualSpacing"]));
    v.push(it(s, "Bordas do parágrafo (lados e cor)", pp("<w:pBdr><w:top w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"FF0000\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"FF0000\"/></w:pBdr>"), |d| com_bloco(d, |b| se(b.atrib.borda == 12 && b.atrib.cor_borda == 0xFF0000FF, format!("lados {} cor {:08X}", b.atrib.borda, b.atrib.cor_borda))), &["<w:pBdr><w:top", "<w:bottom w:val=\"single\"", "w:color=\"FF0000\""]));
    v.push(it(s, "Bordas do parágrafo (tipo e espessura)", pp("<w:pBdr><w:left w:val=\"double\" w:sz=\"12\" w:space=\"4\" w:color=\"0000FF\"/></w:pBdr>"), |d| com_bloco(d, |b| if b.atrib.borda == 1 { p("fica o lado e a cor; o traço duplo de 1,5 pt vira simples de 0,75 pt") } else { f(format!("lados {}", b.atrib.borda)) }), &["<w:left w:val=\"double\" w:sz=\"12\""]).parcial(&["<w:left w:val=\"single\""]));
    v.push(it(s, "Sombreamento do parágrafo", pp("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FFFF99\"/>"), |d| com_bloco(d, |b| se(b.atrib.fundo == 0xFFFF99FF, format!("{:08X}", b.atrib.fundo))), &["w:fill=\"FFFF99\""]));
    v.push(it(s, "Manter com o próximo", pp("<w:keepNext/>"), |d| com_bloco(d, |_| f("o modelo não tem")), &["<w:keepNext"]));
    v.push(it(s, "Manter linhas juntas", pp("<w:keepLines/>"), |d| com_bloco(d, |_| f("o modelo não tem")), &["<w:keepLines"]));
    v.push(it(s, "Controle de linhas órfãs/viúvas (desligado)", pp("<w:widowControl w:val=\"0\"/>"), |d| com_bloco(d, |_| f("o modelo não tem")), &["<w:widowControl w:val=\"0\""]));
    v.push(it(s, "Quebra de página antes", par("", &r("", "um")) + &pp("<w:pageBreakBefore/>"), |d| {
        let i = d.blocos.iter().position(|b| bloco(d).is_some_and(|z| z.id == b.id));
        se(i.is_some_and(|i| i > 0 && matches!(d.blocos[i - 1].conteudo, Conteudo::QuebraDePagina)), "vira um bloco de quebra de página antes")
    }, &["<w:pageBreakBefore"]).parcial(&["w:type=\"page\""]));
    v.push(it(s, "Tabulações personalizadas (posição, alinhamento, preenchimento)", par("<w:tabs><w:tab w:val=\"right\" w:leader=\"dot\" w:pos=\"9000\"/></w:tabs>", &(r("", "Item") + "<w:r><w:tab/></w:r>" + &r("", Z))), |d| com_bloco(d, |b| if b.conteudo.textos().iter().any(|t| texto_puro(t).contains('\t')) { p("o \\t fica no texto; as paradas (posição, alinhamento, pontilhado) somem") } else { f("a tabulação some") }), &["<w:tab w:val=\"right\" w:leader=\"dot\""]).parcial(&["<w:tab/>"]));
    v.push(it(s, "Nível de tópico (outlineLvl) num parágrafo comum", pp("<w:outlineLvl w:val=\"1\"/>"), |d| com_bloco(d, |b| if matches!(b.conteudo, Conteudo::Titulo { nivel: 2, .. }) { p("vira Título 2 (muda a letra)") } else { f(tipo(b)) }), &["<w:outlineLvl w:val=\"1\""]).parcial(&["Heading2"]));
    v.push(it(s, "Letra capitular", par("<w:framePr w:dropCap=\"drop\" w:lines=\"3\" w:wrap=\"around\" w:vAnchor=\"text\" w:hAnchor=\"text\"/>", &r("<w:sz w:val=\"96\"/>", "A")) + &pp(""), |d| f(if d.blocos.len() > 1 { "a capitular vira um parágrafo solto" } else { "a capitular some" }), &["w:dropCap=\"drop\""]));
    v.push(it(s, "Quebra de linha (Shift+Enter)", par("", &(r("", "um") + "<w:r><w:br/></w:r>" + &r("", Z))), |d| com_bloco(d, |b| se(b.conteudo.textos().iter().any(|t| texto_puro(t).contains(QUEBRA_DE_LINHA)), "quebra de linha")), &["<w:br/>"]));

    s = "3. Estilos";
    let destaque = "<w:style w:type=\"paragraph\" w:customStyle=\"1\" w:styleId=\"Destaque\"><w:name w:val=\"Destaque\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:jc w:val=\"center\"/><w:spacing w:before=\"240\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"C00000\"/></w:rPr></w:style>";
    v.push(it(s, "Estilo de parágrafo personalizado (formatação do estilo)", pp("<w:pStyle w:val=\"Destaque\"/>"), |d| com_bloco(d, |b| {
        let r = d.estilos.resolvido(b.atrib.estilo);
        let nome = r.as_ref().filter(|r| r.alinhamento == Alinhamento::Centro && r.bits & NEGRITO != 0 && r.cor == 0xC00000FF).map(|e| e.nome.clone());
        let efeito = b.atrib.alinhamento == Alinhamento::Centro || alvo(d).is_some_and(|t| t.estilo & NEGRITO != 0 || t.fmt.cor != 0);
        match (nome, efeito) {
            (Some(n), _) => x(format!("estilo {n}")),
            (None, true) => p("o efeito sem o estilo"),
            _ => f("nem o estilo nem a formatação dele (centro, negrito, cor)"),
        }
    }), &["styles>w:styleId=\"Destaque\"", "w:pStyle w:val=\"Destaque\""]).partes(&[("word/styles.xml", estilos_com(destaque))]));
    v.push(it(s, "Nome do estilo personalizado na lista de estilos", pp("<w:pStyle w:val=\"Destaque\"/>"), |d| se(d.estilos.lista.iter().any(|e| e.nome == "Destaque"), format!("{} estilos com nome", d.estilos.lista.len())), &["styles><w:name w:val=\"Destaque\""]).partes(&[("word/styles.xml", estilos_com(destaque))]));
    v.push(it(s, "Estilo de caractere (rStyle)", pr("<w:rStyle w:val=\"Enfase\"/>"), |d| com_alvo(d, |t| match d.estilos.pegar(t.fmt.estilo_de_caractere) {
        Some(e) if e.caractere && e.nome == "Ênfase forte" && t.estilo & ITALICO != 0 && t.fmt.cor == 0x7030A0FF => x("estilo Ênfase forte (itálico roxo)"),
        _ if t.estilo & ITALICO != 0 => p("o efeito sem o estilo"),
        _ => f("nem o estilo nem o itálico dele"),
    }), &["styles>w:styleId=\"Enfase\"", "w:rStyle w:val=\"Enfase\""]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"character\" w:styleId=\"Enfase\"><w:name w:val=\"Ênfase forte\"/><w:rPr><w:i/><w:color w:val=\"7030A0\"/></w:rPr></w:style>"))]));
    v.push(it(s, "Herança de estilos (basedOn)", pp("<w:pStyle w:val=\"Filho\"/>"), |d| com_alvo(d, |t| match (t.estilo & NEGRITO != 0, t.fmt.cor == 0x00B050FF) {
        _ if bloco(d).and_then(|b| d.estilos.do_bloco(b)).is_some_and(|e| e.nome == "Filho" && d.estilos.pegar(e.herda).is_some_and(|p| p.nome == "Pai")) => {
            let r = bloco(d).and_then(|b| d.estilos.resolvido_do_bloco(b));
            se(r.is_some_and(|r| r.bits & NEGRITO != 0 && r.cor == 0x00B050FF), "Filho herda de Pai: negrito do pai, verde do filho")
        }
        (true, true) => p("o efeito somado sem os estilos"),
        (false, false) => f("nem o negrito do pai nem a cor do filho"),
        _ => p("só parte do efeito"),
    }), &["styles><w:basedOn w:val=\"Pai\""]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"paragraph\" w:styleId=\"Pai\"><w:name w:val=\"Pai\"/><w:basedOn w:val=\"Normal\"/><w:rPr><w:b/></w:rPr></w:style><w:style w:type=\"paragraph\" w:styleId=\"Filho\"><w:name w:val=\"Filho\"/><w:basedOn w:val=\"Pai\"/><w:rPr><w:color w:val=\"00B050\"/></w:rPr></w:style>"))]));
    v.push(it(s, "Padrão do documento (docDefaults: fonte e tamanho)", pp(""), |d| com_alvo(d, |t| {
        let e = &d.estilos.padrao;
        match (e.familia.nome() == "Georgia" && e.tamanho == 12.0, t.fmt.tamanho == 12.0 || t.estilo & SERIFA != 0) {
            (true, _) => se(bloco(d).and_then(|b| d.estilos.resolvido_do_bloco(b)).is_some_and(|r| r.bits & SERIFA != 0 && r.tamanho == 12.0), "Georgia 12 pt no padrão, e o parágrafo com eles"),
            (_, true) => p("fonte ou tamanho do padrão no trecho"),
            _ => f("nem a Georgia nem os 12 pt do padrão (grava Arial 11 fixo)"),
        }
    }), &["styles><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Georgia\"", "styles><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\" w:cs=\"Georgia\"/><w:sz w:val=\"24\""]).partes(&[("word/styles.xml", estilos_padrao().replacen("<w:style ", "<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\"/><w:sz w:val=\"24\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"200\" w:line=\"360\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults><w:style ", 1))]));
    v.push(it(s, "Estilo Normal modificado", pp(""), |d| com_bloco(d, |b| {
        let corpo = d.estilos.resolvido_do_bloco(b).is_some_and(|e| e.tamanho == 13.0) || alvo(d).is_some_and(|t| t.fmt.tamanho == 13.0);
        match (b.atrib.alinhamento == Alinhamento::Justificado, corpo) {
            (true, true) => x("justificado, 13 pt"),
            (false, false) => f("nem o justificado nem os 13 pt do Normal"),
            _ => p("só o justificado ou só os 13 pt"),
        }
    }), &["styles><w:jc w:val=\"both\""]).partes(&[("word/styles.xml", estilos_padrao().replace("<w:name w:val=\"Normal\"/>", "<w:name w:val=\"Normal\"/><w:pPr><w:jc w:val=\"both\"/></w:pPr><w:rPr><w:sz w:val=\"26\"/></w:rPr>"))]));
    v.push(it(s, "Títulos internos (Heading 1–9)", pp("<w:pStyle w:val=\"Ttulo2\"/>"), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Titulo { nivel: 2, .. }), tipo(b))), &["w:pStyle w:val=\"Heading2\""]));
    v.push(it(s, "Aparência do estilo de título (cor e tamanho do styles.xml)", pp("<w:pStyle w:val=\"Ttulo1\"/>"), |d| com_alvo(d, |t| match bloco(d).and_then(|b| d.estilos.resolvido_do_bloco(b)) {
        Some(e) if e.cor == 0x2F5496FF && e.tamanho == 20.0 => x("o título com a cor e os 20 pt do styles.xml"),
        _ if t.fmt.cor == 0x2F5496FF || d.estilos.lista.iter().any(|e| e.cor == 0x2F5496FF) => p("a cor está no modelo, mas não no título"),
        _ => f("a letra do título é a do FeatherDocs"),
    }), &["styles>w:val=\"2F5496\""]).partes(&[("word/styles.xml", estilos_padrao().replace("<w:outlineLvl w:val=\"0\"/></w:pPr>", "<w:outlineLvl w:val=\"0\"/></w:pPr><w:rPr><w:color w:val=\"2F5496\"/><w:sz w:val=\"40\"/></w:rPr>"))]));
    v.push(it(s, "Estilo Título (Title) e Subtítulo", pp("<w:pStyle w:val=\"Ttulo\"/>"), |d| com_bloco(d, |b| match (&b.conteudo, d.estilos.do_bloco(b)) {
        (Conteudo::Titulo { nivel: 1, .. }, Some(e)) if e.codigo == "Title" => x("Título 1 com o estilo Title"),
        (Conteudo::Titulo { nivel: 1, .. }, _) => p("vira Título 1"),
        _ => f(tipo(b)),
    }), &["w:pStyle w:val=\"Title\""]).parcial(&["w:pStyle w:val=\"Heading1\""]));
    v.push(it(s, "Estilo Citação (Quote)", pp("<w:pStyle w:val=\"Citao\"/>"), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Citacao { .. }), tipo(b))), &["w:pStyle w:val=\"Quote\""]));
    v.push(it(s, "Estilo com nome criado no FeatherDocs (FDEstilo)", pp("<w:pStyle w:val=\"FDEstilo1\"/>"), |d| com_bloco(d, |b| {
        let e = d.estilos.pegar(b.atrib.estilo);
        se(e.is_some_and(|e| e.nome == "Resumo" && e.alinhamento == Alinhamento::Centro && e.bits & ITALICO != 0), format!("{:?}", e.map(|e| (&e.nome, e.alinhamento, e.bits))))
    }), &["styles>w:styleId=\"FDEstilo1\"", "w:pStyle w:val=\"FDEstilo1\""]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"paragraph\" w:customStyle=\"1\" w:styleId=\"FDEstilo1\"><w:name w:val=\"Resumo\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:i/></w:rPr></w:style>"))]));
    v.push(it(s, "Estilo de tabela (tblStyle)", tabela2("<w:tblStyle w:val=\"GradeColorida\"/><w:tblLook w:val=\"04A0\" w:firstRow=\"1\"/>", "", ""), |d| f(if tabela_de(d).is_some() { "o modelo não tem estilo de tabela" } else { "sem tabela" }), &["w:tblStyle w:val=\"GradeColorida\""]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"table\" w:styleId=\"GradeColorida\"><w:name w:val=\"Grade Colorida\"/><w:tblPr><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:color=\"4472C4\"/></w:tblBorders></w:tblPr><w:tblStylePr w:type=\"firstRow\"><w:rPr><w:b/></w:rPr><w:tcPr><w:shd w:val=\"clear\" w:fill=\"4472C4\"/></w:tcPr></w:tblStylePr></w:style>"))]));

    s = "4. Listas";
    v.push(it(s, "Lista com marcadores", item(2, 0, Z), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Item { ordenado: false, .. }), tipo(b))), &["<w:numPr>", "numbering>w:numFmt w:val=\"bullet\""]));
    v.push(it(s, "Lista numerada", item(1, 0, Z), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Item { ordenado: true, .. }), tipo(b))), &["<w:numPr>", "numbering>w:numFmt w:val=\"decimal\""]));
    v.push(it(s, "Multinível (subitem)", item(1, 0, "um") + &item(1, 1, Z), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Item { nivel: 1, .. }), format!("{} {:?}", tipo(b), b.conteudo.textos().len()))), &["<w:ilvl w:val=\"1\"/>"]));
    v.push(it(s, "Formato do número por nível (a), i.)", item(1, 0, "um") + &item(1, 1, Z), |d| com_bloco(d, |b| se(b.atrib.lista == Lista::Alfabetica, format!("{:?}", b.atrib.lista))), &["numbering>w:numFmt w:val=\"lowerLetter\""]));
    v.push(it(s, "Letra ou romano no primeiro nível", item(10, 0, Z), |d| com_bloco(d, |b| se(b.atrib.lista == Lista::Alfabetica, format!("{:?}", b.atrib.lista))), &["numbering>w:numFmt w:val=\"lowerLetter\""]));
    v.push(it(s, "Texto do número personalizado (\"Passo 1:\")", item(7, 0, Z), |d| com_bloco(d, |b| if matches!(b.conteudo, Conteudo::Item { ordenado: true, .. }) { p("vira \"1.\"") } else { f(tipo(b)) }), &["numbering>w:lvlText w:val=\"Passo %1:\""]).parcial(&["<w:numPr>"]));
    v.push(it(s, "Início da numeração em outro número", item(5, 0, Z), |d| com_bloco(d, |b| if matches!(b.conteudo, Conteudo::Item { ordenado: true, .. }) { f("começa em 1 (o modelo não tem o número inicial)") } else { f(tipo(b)) }), &["numbering><w:start w:val=\"5\""]));
    v.push(it(s, "Reinício da numeração (startOverride)", item(1, 0, "um") + &item(1, 0, "dois") + &item(3, 0, Z), |d| com_bloco(d, |b| se(b.atrib.reiniciar, format!("reiniciar {}", b.atrib.reiniciar))), &["numbering><w:startOverride w:val=\"1\""]));
    v.push(it(s, "Continuar a numeração depois de um parágrafo", item(1, 0, "um") + &pp("").replace(Z, "meio") + &item(1, 0, Z), |d| com_bloco(d, |b| f(format!("{}: a lista é a sequência de itens vizinhos, recomeça em 1", tipo(b)))), &["!numbering><w:startOverride"]));
    v.push(it(s, "Marcador de outro caractere (➢)", item(4, 0, Z), |d| com_bloco(d, |b| se(b.atrib.lista == Lista::Seta, format!("{:?}", b.atrib.lista))), &["numbering>w:lvlText w:val=\"➢\""]));
    v.push(it(s, "Marcador de símbolo (Wingdings)", item(8, 0, Z), |d| com_bloco(d, |b| if matches!(b.conteudo, Conteudo::Item { ordenado: false, .. }) { p(format!("vira o marcador {:?}", b.atrib.lista)) } else { f(tipo(b)) }), &["numbering>w:ascii=\"Wingdings\""]).parcial(&["numbering>w:numFmt w:val=\"bullet\""]));
    v.push(it(s, "Lista de tarefas (☐)", item(9, 0, Z), |d| com_bloco(d, |b| se(b.atrib.lista == Lista::Tarefa, format!("{:?}", b.atrib.lista))), &["numbering>w:lvlText w:val=\"☐\""]));
    v.push(it(s, "Estilo de lista (numStyleLink)", item(6, 0, Z), |d| com_bloco(d, |b| match &b.conteudo {
        Conteudo::Item { ordenado: true, .. } => x("numerada, como o estilo"),
        Conteudo::Item { .. } => p("vira lista com marcadores (o estilo de lista não é seguido)"),
        _ => f(tipo(b)),
    }), &["numbering><w:numStyleLink"]).parcial(&["<w:numPr>"]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"numbering\" w:styleId=\"MinhaLista\"><w:name w:val=\"Minha lista\"/><w:pPr><w:numPr><w:numId w:val=\"1\"/></w:numPr></w:pPr></w:style>"))]));
    v.push(it(s, "Numeração vinda do estilo de parágrafo (List Number)", pp("<w:pStyle w:val=\"ListaNum\"/>"), |d| com_bloco(d, |b| se(matches!(b.conteudo, Conteudo::Item { ordenado: true, .. }), if matches!(b.conteudo, Conteudo::Item { .. }) { tipo(b) } else { format!("vira {}: o numPr do estilo não é seguido", tipo(b)) })), &["<w:numPr>"]).partes(&[("word/styles.xml", estilos_com("<w:style w:type=\"paragraph\" w:styleId=\"ListaNum\"><w:name w:val=\"List Number\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:numPr><w:numId w:val=\"1\"/></w:numPr></w:pPr></w:style>"))]));
    v.push(it(s, "Recuo próprio do item", pp("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"2\"/></w:numPr><w:ind w:left=\"1440\" w:hanging=\"360\"/>"), |d| com_bloco(d, |b| se(b.atrib.recuo_esq == 36.0, format!("recuo além do nível {}", b.atrib.recuo_esq))), &["w:left=\"1440\""]));

    s = "5. Tabelas";
    v.push(it(s, "Tabela simples", tabela2("", "", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(_, _, l)| se(l.len() == 2 && l[1].len() == 2 && texto_puro(&l[1][0]) == Z, format!("{}×{}", l.len(), l[0].len()))), &["<w:tbl>", "<w:tc>"]));
    v.push(it(s, "Linha de cabeçalho repetida", tabela2("", "<w:tblHeader/>", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(_, c, _)| se(c, format!("cabeçalho {c}"))), &["<w:tblHeader/>"]));
    v.push(it(s, "Mescla horizontal (gridSpan)", tabela("", "<w:gridCol w:w=\"3000\"/><w:gridCol w:w=\"3000\"/><w:gridCol w:w=\"3000\"/>", &[tr("", &[tc("<w:gridSpan w:val=\"2\"/>", &par("", &r("", Z))), cel("C")]), tr("", &[cel("1"), cel("2"), cel("3")])]), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(b, _, l)| se(b.mesclagens.iter().any(|m| m.colunas == 2), if b.mesclagens.is_empty() { format!("o gridSpan não é lido: a 1ª linha fica com {} células e a 2ª com {} (colunas desalinhadas)", l[0].len(), l.get(1).map_or(0, Vec::len)) } else { format!("{} mesclagem(ns)", b.mesclagens.len()) })), &["<w:gridSpan w:val=\"2\""]).montar(|| {
        let mut d = Documento::vazio();
        let id = d.acrescentar(Conteudo::Tabela { cabecalho: false, linhas: vec![vec![vec![Trecho::simples(Z)], vec![], vec![Trecho::simples("C")]], vec![vec![Trecho::simples("1")], vec![Trecho::simples("2")], vec![Trecho::simples("3")]]] });
        d.blocos.iter_mut().find(|b| b.id == id).unwrap().mesclagens.push(MesclagemTabela { linha: 0, coluna: 0, colunas: 2 });
        d
    }));
    v.push(it(s, "Mescla vertical (vMerge)", tabela("", "<w:gridCol w:w=\"4000\"/><w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("<w:vMerge w:val=\"restart\"/>", &par("", &r("", Z))), cel("B")]), tr("", &[tc("<w:vMerge/>", "<w:p/>"), cel("2")])]), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo só tem mescla horizontal")), &["<w:vMerge"]));
    v.push(it(s, "Larguras das colunas", tabela2("", "", "").replace("<w:gridCol w:w=\"4000\"/><w:gridCol w:w=\"4000\"/>", "<w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"6000\"/>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem largura de coluna (grava todas iguais)")), &["<w:gridCol w:w=\"2000\"/>"]));
    v.push(it(s, "Largura da tabela (porcentagem)", tabela2("<w:tblW w:w=\"2500\" w:type=\"pct\"/>", "", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem; grava a largura útil inteira")), &["w:type=\"pct\""]));
    v.push(it(s, "Alinhamento da tabela na página", tabela2("<w:jc w:val=\"center\"/>", "", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(b, _, _)| se(b.atrib.alinhamento == Alinhamento::Centro, format!("{:?}", b.atrib.alinhamento))), &["<w:tblPr><w:jc w:val=\"center\""]));
    v.push(it(s, "Autoajuste / layout fixo", tabela2("<w:tblLayout w:type=\"fixed\"/>", "", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:tblLayout"]));
    v.push(it(s, "Bordas da tabela (tblBorders)", tabela2("<w:tblBorders><w:top w:val=\"double\" w:sz=\"8\" w:color=\"FF0000\"/><w:bottom w:val=\"double\" w:sz=\"8\" w:color=\"FF0000\"/></w:tblBorders>", "", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem; grava a grade cinza padrão")), &["w:val=\"double\" w:sz=\"8\" w:color=\"FF0000\""]));
    v.push(it(s, "Bordas por célula (tcBorders)", tabela2("", "", "<w:tcBorders><w:bottom w:val=\"single\" w:sz=\"12\" w:color=\"00B050\"/></w:tcBorders>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:tcBorders>"]));
    v.push(it(s, "Sombreamento de célula", tabela2("", "", "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FFC000\"/>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem (só o cinza fixo do cabeçalho)")), &["w:fill=\"FFC000\""]));
    v.push(it(s, "Alinhamento vertical da célula", tabela2("", "", "<w:vAlign w:val=\"center\"/>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:vAlign w:val=\"center\""]));
    v.push(it(s, "Alinhamento do texto na célula", tabela("", "<w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("", &pp("<w:jc w:val=\"right\"/>"))])]), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("a célula é só texto: o parágrafo perde os atributos")), &["<w:jc w:val=\"right\""]));
    v.push(it(s, "Altura da linha", tabela2("", "<w:trHeight w:val=\"800\" w:hRule=\"exact\"/>", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:trHeight"]));
    v.push(it(s, "Direção do texto na célula (vertical)", tabela2("", "", "<w:textDirection w:val=\"btLr\"/>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:textDirection"]));
    v.push(it(s, "Formatação de caractere na célula", tabela("", "<w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("", &par("", &r("<w:b/><w:color w:val=\"FF0000\"/>", Z)))])]), |d| com_alvo(d, |t| se(t.estilo & NEGRITO != 0 && t.fmt.cor == 0xFF0000FF, "negrito vermelho")), &["<w:tc>", "<w:b/>", "w:color w:val=\"FF0000\""]));
    v.push(it(s, "Vários parágrafos numa célula", tabela("", "<w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("", &(par("", &r("", "um")) + &par("", &r("", Z))))])]), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(_, _, l)| if texto_puro(&l[0][0]).contains("um") && texto_puro(&l[0][0]).contains(Z) { p(format!("viram um só, separados por espaço: {:?}", texto_puro(&l[0][0]))) } else { f("texto perdido") }), &["um</w:t></w:r></w:p><w:p>"]).parcial(&["um</w:t>"]));
    v.push(it(s, "Lista dentro de célula", tabela("", "<w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("", &item(2, 0, Z))])]), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |(_, _, l)| if texto_puro(&l[0][0]).contains(Z) { p("o texto fica, o marcador some") } else { f("texto perdido") }), &["</w:tcPr><w:p><w:pPr><w:pStyle w:val=\"ListParagraph\"/><w:numPr>"]).parcial(&["ZQX</w:t>"]));
    v.push(it(s, "Imagem dentro de célula", tabela("", "<w:gridCol w:w=\"4000\"/>", &[tr("", &[tc("", &(img_par(&desenho(false, "", "", "")) + &par("", &r("", Z))))])]), |d| se(!imagens(d).is_empty(), if imagens(d).is_empty() { "a imagem some (a célula guarda só texto)" } else { "imagem" }), &["<wp:inline"]));
    v.push(it(s, "Tabela aninhada", tabela("", "<w:gridCol w:w=\"6000\"/>", &[tr("", &[tc("", &(tabela2("", "", "") + "<w:p/>"))])]), |d| {
        let n = d.blocos.iter().filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. })).count();
        if n == 0 { f("sem tabela") } else if texto_todo(d).contains(Z) { p("a tabela de dentro vira texto na célula de fora") } else { f("a tabela de dentro some") }
    }, &["</w:tcPr><w:tbl>"]).parcial(&["ZQX</w:t>"]));
    v.push(it(s, "Margens da célula (tcMar)", tabela2("", "", "<w:tcMar><w:left w:w=\"300\" w:type=\"dxa\"/></w:tcMar>"), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("o modelo não tem")), &["<w:tcMar>"]));

    s = "6. Imagens";
    v.push(it(s, "Imagem em linha (embutida)", img_par(&desenho(false, "", "", "")) + &pp(""), |d| imagens(d).first().map_or_else(|| f("sem imagem"), |b| se(matches!(&b.conteudo, Conteudo::Imagem { origem, .. } if origem.starts_with("data:image/png")), "imagem data:")), &["<wp:inline", "r:embed=", "[Content_Types]>Extension=\"png\""]));
    v.push(it(s, "Tamanho da imagem", img_par(&desenho(false, "", "", "")), |d| imagens(d).first().map_or_else(|| f("sem imagem"), |b| if let Conteudo::Imagem { largura, altura, .. } = &b.conteudo { se(*largura == 180.0 && *altura == 90.0, format!("{}×{} pt", mm(*largura), mm(*altura))) } else { f("") }), &["<wp:extent cx=\"2286000\" cy=\"1143000\"/>"]));
    v.push(it(s, "Texto alternativo (descr)", img_par(&desenho(false, " descr=\"Logo da empresa\"", "", "")), |d| se(texto_todo(d).contains("Logo da empresa"), if imagens(d).is_empty() { "sem imagem" } else { "o modelo não tem texto alternativo" }), &["descr=\"Logo da empresa\""]));
    v.push(it(s, "Imagem flutuante (âncora, posição, quebra automática)", img_par(&desenho(true, "", "", "")) + &pp(""), |d| if imagens(d).is_empty() { f("sem imagem") } else { p("vira imagem em linha num bloco próprio: perde posição e quebra do texto") }, &["<wp:anchor", "<wp:wrapSquare"]).parcial(&["<wp:inline"]));
    v.push(it(s, "Imagem no meio do texto", par("", &(r("", "antes ") + &format!("<w:r>{}</w:r>", desenho(false, "", "", "")) + &r("", Z))), |d| {
        if marcas(d).iter().any(|m| matches!(m, Marca::Imagem { .. })) { x("imagem presa ao texto") } else if !imagens(d).is_empty() { p("sai do parágrafo e vira bloco próprio depois dele") } else { f("sem imagem") }
    }, &["antes </w:t></w:r><w:r><w:drawing><wp:inline"]).parcial(&["<wp:inline"]).montar(|| {
        let mut d = Documento::vazio();
        let origem = base64::para_data_url("image/png", PNG);
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("antes "), Trecho::objeto(Marca::Imagem { origem, largura: 12.0, altura: 12.0 }), Trecho::simples(Z)] });
        d
    }));
    v.push(it(s, "Recorte (srcRect)", img_par(&desenho(false, "", "<a:srcRect l=\"10000\" t=\"5000\"/>", "")), |d| if imagens(d).is_empty() { f("sem imagem") } else { f("o modelo não tem recorte: a imagem volta inteira") }, &["<a:srcRect"]));
    v.push(it(s, "Rotação e espelhamento", img_par(&desenho(false, "", "", " rot=\"5400000\" flipH=\"1\"")), |d| if imagens(d).is_empty() { f("sem imagem") } else { f("o modelo não tem rotação") }, &["rot=\"5400000\""]));
    v.push(it(s, "Imagem ligada (endereço da web)", img_par(&desenho(false, "", "", "").replace("r:embed=\"rIdImg\"", "r:link=\"rIdImgWeb\"")), |d| imagens(d).first().map_or_else(|| f("sem imagem"), |b| se(matches!(&b.conteudo, Conteudo::Imagem { origem, .. } if origem == "http://exemplo.org/foto.png"), "endereço")), &["r:link=", "rels>http://exemplo.org/foto.png"]));
    v.push(it(s, "Imagem VML antiga (w:pict/v:imagedata)", "<w:p><w:r><w:pict><v:shape style=\"width:100pt;height:50pt\"><v:imagedata r:id=\"rIdImg\"/></v:shape></w:pict></w:r></w:p>".to_string(), |d| se(!imagens(d).is_empty(), format!("{} imagens", imagens(d).len())), &["<wp:inline"]));

    s = "7. Formas, caixas de texto e objetos";
    let caixa = format!("<w:p><w:r><mc:AlternateContent><mc:Choice Requires=\"wps\"><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"1\" behindDoc=\"0\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"column\"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"paragraph\"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx=\"1828800\" cy=\"457200\"/><wp:wrapSquare wrapText=\"bothSides\"/><wp:docPr id=\"5\" name=\"Caixa de texto 5\"/><a:graphic><a:graphicData uri=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:wsp><wps:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>{}</w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect style=\"width:144pt;height:36pt\"><v:textbox><w:txbxContent>{}</w:txbxContent></v:textbox></v:rect></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>fora</w:t></w:r></w:p>", par("", &r("", Z)), par("", &r("", Z)));
    v.push(it(s, "Caixa de texto", caixa, |d| match quantos_z(d) {
        0 => f("o texto da caixa some"),
        1 => p("o texto vira parágrafo solto; a caixa some"),
        n => p(format!("o texto vira parágrafo solto, {n} vezes (lê a Choice e a Fallback); a caixa some")),
    }, &["<wps:txbx", "<w:txbxContent>"]).parcial(&["ZQX</w:t>"]));
    v.push(it(s, "Forma (retângulo, seta…)", "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"914400\" cy=\"457200\"/><wp:docPr id=\"6\" name=\"Seta\"/><a:graphic><a:graphicData uri=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:wsp><wps:spPr><a:prstGeom prst=\"rightArrow\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"FF0000\"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>".to_string(), |d| na_sacola(d).unwrap_or_else(|| se(d.blocos.iter().any(|b| !matches!(b.conteudo, Conteudo::Paragrafo { .. })), "o modelo não tem forma")), &["prst=\"rightArrow\""]));
    v.push(it(s, "Gráfico (c:chart)", "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"4572000\" cy=\"2743200\"/><wp:docPr id=\"7\" name=\"Gráfico 1\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart r:id=\"rIdGraf\"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>".to_string(), |d| na_sacola(d).unwrap_or_else(|| if imagens(d).is_empty() { f("o gráfico some") } else { p("vira imagem") }), &["<c:chart"]).partes(&[("word/charts/chart1.xml", format!("<c:chartSpace {NS}><c:chart><c:plotArea><c:barChart><c:ser><c:tx><c:v>Vendas</c:v></c:tx><c:val><c:numLit><c:pt idx=\"0\"><c:v>3</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"))]));
    v.push(it(s, "SmartArt", "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"4572000\" cy=\"2743200\"/><wp:docPr id=\"8\" name=\"Diagrama 1\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\"><dgm:relIds r:dm=\"rIdDm\" r:lo=\"rIdLo\" r:qs=\"rIdQs\" r:cs=\"rIdCs\"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>".to_string(), |d| na_sacola(d).unwrap_or_else(|| if imagens(d).is_empty() { f("o SmartArt some") } else { p("vira imagem") }), &["<dgm:relIds"]));
    v.push(it(s, "Objeto OLE (planilha incorporada)", "<w:p><w:r><w:object w:dxaOrig=\"2000\" w:dyaOrig=\"1000\"><v:shape style=\"width:100pt;height:50pt\"><v:imagedata r:id=\"rIdImg\"/></v:shape><o:OLEObject Type=\"Embed\" ProgID=\"Excel.Sheet.12\" r:id=\"rIdOle\"/></w:object></w:r></w:p>".to_string(), |d| na_sacola(d).unwrap_or_else(|| if imagens(d).is_empty() { f("o objeto some (nem a prévia fica)") } else { p("fica só a prévia") }), &["<o:OLEObject"]).parcial(&["<wp:inline"]));
    v.push(it(s, "Controle de conteúdo de texto (sdt)", "<w:sdt><w:sdtPr><w:alias w:val=\"Nome\"/><w:tag w:val=\"nome\"/><w:text/></w:sdtPr><w:sdtContent>".to_string() + &pp("") + "</w:sdtContent></w:sdt>", |d| controle_na_sacola(d, "w:alias").filter(|_| alvo(d).is_some()).unwrap_or_else(|| if alvo(d).is_some() { p("o texto fica, o controle (título, marca, tipo) some") } else { sumiu() }), &["<w:sdt>", "<w:alias w:val=\"Nome\""]));
    v.push(it(s, "Caixa de seleção (sdt checkbox)", par("", &format!("<w:sdt><w:sdtPr><w14:checkbox><w14:checked w14:val=\"1\"/></w14:checkbox></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>{}", r("<w:rFonts w:ascii=\"MS Gothic\"/>", "☒"), r("", " ZQX"))), |d| controle_na_sacola(d, "w14:checkbox").filter(|_| texto_todo(d).contains('☒')).unwrap_or_else(|| if texto_todo(d).contains('☒') { p("fica o caractere ☒, sem a caixa clicável") } else { f("some") }), &["<w14:checkbox"]).parcial(&["☒"]));
    v.push(it(s, "Lista suspensa (sdt dropDownList)", par("", &format!("<w:sdt><w:sdtPr><w:dropDownList><w:listItem w:displayText=\"Sim\" w:value=\"1\"/><w:listItem w:displayText=\"Não\" w:value=\"2\"/></w:dropDownList></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>", r("", Z))), |d| controle_na_sacola(d, "w:dropDownList").filter(|_| alvo(d).is_some()).unwrap_or_else(|| if alvo(d).is_some() { p("fica o valor escolhido, as opções somem") } else { sumiu() }), &["<w:dropDownList"]));
    v.push(it(s, "Seletor de data (sdt date)", par("", &format!("<w:sdt><w:sdtPr><w:date w:fullDate=\"2026-10-28T00:00:00Z\"><w:dateFormat w:val=\"dd/MM/yyyy\"/></w:date></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>", r("", Z))), |d| controle_na_sacola(d, "w:date").filter(|_| alvo(d).is_some()).unwrap_or_else(|| if alvo(d).is_some() { p("fica a data como texto, o seletor some") } else { sumiu() }), &["<w:date "]));

    s = "8. Seções e página";
    v.push(it(s, "Tamanho do papel (A4)", pp("") + &sect(""), |d| se(d.pagina.largura == ConfigPagina::a4().largura && d.pagina.altura == ConfigPagina::a4().altura, format!("{}×{}", mm(d.pagina.largura), mm(d.pagina.altura))), &["<w:pgSz w:w=\"11906\" w:h=\"16838\""]));
    v.push(it(s, "Papel de outro tamanho (A5)", pp("") + &sect("").replace("w:w=\"11906\" w:h=\"16838\"", "w:w=\"8391\" w:h=\"11906\""), |d| se((d.pagina.largura - 419.55).abs() < 0.1, format!("{}×{}", mm(d.pagina.largura), mm(d.pagina.altura))), &["<w:pgSz w:w=\"8391\" w:h=\"11906\""]));
    v.push(it(s, "Orientação paisagem", pp("") + &sect("").replace("w:w=\"11906\" w:h=\"16838\"", "w:w=\"16838\" w:h=\"11906\" w:orient=\"landscape\""), |d| se(d.pagina.largura > d.pagina.altura, format!("{}×{}", mm(d.pagina.largura), mm(d.pagina.altura))), &["w:orient=\"landscape\""]));
    v.push(it(s, "Margens", pp("") + &sect(""), |d| se((d.pagina.margem_esq - 85.05).abs() < 0.01 && (d.pagina.margem_sup - 70.85).abs() < 0.01, format!("sup {} esq {}", d.pagina.margem_sup, d.pagina.margem_esq)), &["w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\""]));
    v.push(it(s, "Medianiz", pp("") + &sect("").replace("w:gutter=\"0\"", "w:gutter=\"567\""), |_| f("o modelo não tem medianiz"), &["w:gutter=\"567\""]));
    v.push(it(s, "Distância do cabeçalho e do rodapé à borda", pp("") + &sect("").replace("w:header=\"708\" w:footer=\"708\"", "w:header=\"1000\" w:footer=\"900\""), |_| f("o modelo não tem; grava 708 fixo"), &["w:header=\"1000\""]));
    v.push(it(s, "Colunas de texto", pp("") + &sect("<w:cols w:num=\"2\" w:space=\"720\"/>").replace("<w:cols w:num=\"2\" w:space=\"720\"/><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/>", "<w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/><w:cols w:num=\"2\" w:space=\"720\"/>"), |_| f("o modelo não tem número de colunas"), &["<w:cols w:num=\"2\""]));
    v.push(it(s, "Quebra de seção (próxima página)", pp("<w:sectPr><w:type w:val=\"nextPage\"/></w:sectPr>") + &par("", &r("", "depois")), |d| se(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDeSecao { inicio: InicioDeSecao::ProximaPagina })), "quebra de seção"), &["<w:type w:val=\"nextPage\""]));
    v.push(it(s, "Quebra de seção contínua", pp("<w:sectPr><w:type w:val=\"continuous\"/></w:sectPr>") + &par("", &r("", "depois")), |d| se(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDeSecao { inicio: InicioDeSecao::Continua })), "contínua"), &["<w:type w:val=\"continuous\""]));
    v.push(it(s, "Quebra de seção em página ímpar/par", pp("<w:sectPr><w:type w:val=\"oddPage\"/></w:sectPr>") + &par("", &r("", "depois")), |d| se(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDeSecao { inicio: InicioDeSecao::PaginaImpar })), "ímpar"), &["<w:type w:val=\"oddPage\""]));
    v.push(it(s, "Várias seções com papel próprio (retrato e paisagem)", pp(&sect("<w:type w:val=\"nextPage\"/>")) + &par("", &r("", "depois")) + &sect("").replace("w:w=\"11906\" w:h=\"16838\"", "w:w=\"16838\" w:h=\"11906\" w:orient=\"landscape\""), |d| {
        let quebra = d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDeSecao { .. }));
        if quebra { p(format!("a quebra fica; o papel é um só no documento ({}×{})", mm(d.pagina.largura), mm(d.pagina.altura))) } else { f("sem quebra de seção") }
    }, &["<w:pPr><w:sectPr>", "w:w=\"11906\" w:h=\"16838\"/><w:pgMar", "w:orient=\"landscape\""]).parcial(&["<w:pPr><w:sectPr>"]));
    v.push(it(s, "Alinhamento vertical da página", pp("") + &sect("<w:vAlign w:val=\"center\"/>").replace("<w:vAlign w:val=\"center\"/><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/>", "<w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/><w:vAlign w:val=\"center\"/>"), |d| se(d.extras.vertical == Vertical::Centro, format!("{:?}", d.extras.vertical)), &["<w:vAlign w:val=\"center\""]));
    v.push(it(s, "Número da primeira página (pgNumType start)", pp("") + &sect("").replace("</w:sectPr>", "<w:pgNumType w:start=\"5\"/></w:sectPr>"), |d| se(d.extras.pagina_inicial == 5, format!("começa em {}", d.extras.pagina_inicial)), &["<w:pgNumType w:start=\"5\""]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples(Z)] });
        d.extras.pagina_inicial = 5;
        d
    }));
    v.push(it(s, "Formato do número de página (romano)", pp("") + &sect("").replace("</w:sectPr>", "<w:pgNumType w:fmt=\"lowerRoman\"/></w:sectPr>"), |_| f("o modelo não tem formato por seção"), &["<w:pgNumType w:fmt=\"lowerRoman\""]));
    v.push(it(s, "Quebra de página manual", par("", &r("", "um")) + "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>" + &pp(""), |d| se(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDePagina)), "quebra de página"), &["<w:br w:type=\"page\"/>"]));
    v.push(it(s, "Quebra de coluna", par("", &r("", "um")) + "<w:p><w:r><w:br w:type=\"column\"/></w:r></w:p>" + &pp(""), |d| se(d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::QuebraDeColuna)), "quebra de coluna"), &["<w:br w:type=\"column\"/>"]));
    v.push(it(s, "Cor da página", "<w:background w:color=\"FFF2CC\"/>".to_string() + &pp(""), |d| se(d.extras.cor_pagina == 0xFFF2CCFF, format!("{:08X}", d.extras.cor_pagina)), &["<w:background w:color=\"FFF2CC\""]));
    v.push(it(s, "Borda da página", pp("") + &sect("").replace("</w:sectPr>", "<w:pgBorders w:offsetFrom=\"page\"><w:top w:val=\"single\" w:sz=\"12\" w:space=\"24\" w:color=\"2E74B5\"/><w:left w:val=\"single\" w:sz=\"12\" w:space=\"24\" w:color=\"2E74B5\"/><w:bottom w:val=\"single\" w:sz=\"12\" w:space=\"24\" w:color=\"2E74B5\"/><w:right w:val=\"single\" w:sz=\"12\" w:space=\"24\" w:color=\"2E74B5\"/></w:pgBorders></w:sectPr>"), |d| se(d.extras.borda_cor == 0x2E74B5FF && d.extras.borda_espessura == 1.5 && d.extras.borda_distancia == 24.0, format!("{:08X} {} {}", d.extras.borda_cor, d.extras.borda_espessura, d.extras.borda_distancia)), &["<w:pgBorders", "w:sz=\"12\" w:space=\"24\" w:color=\"2E74B5\""]));
    v.push(it(s, "Borda da página com arte ou traço duplo", pp("") + &sect("").replace("</w:sectPr>", "<w:pgBorders><w:top w:val=\"double\" w:sz=\"6\" w:space=\"24\" w:color=\"000000\"/></w:pgBorders></w:sectPr>"), |d| if d.extras.borda_cor != 0 { p("vira traço simples nos quatro lados") } else { f("sem borda") }, &["<w:top w:val=\"double\""]).parcial(&["<w:pgBorders"]));
    v.push(it(s, "Números de linha (contínuos)", pp("") + &sect("").replace("</w:sectPr>", "<w:lnNumType w:countBy=\"1\" w:restart=\"continuous\"/></w:sectPr>"), |d| se(d.extras.numerar_linhas == NumeracaoDeLinhas::Continua, format!("{:?}", d.extras.numerar_linhas)), &["<w:lnNumType w:countBy=\"1\" w:restart=\"continuous\""]));
    v.push(it(s, "Números de linha (por página, de 5 em 5)", pp("") + &sect("").replace("</w:sectPr>", "<w:lnNumType w:countBy=\"5\" w:restart=\"newPage\"/></w:sectPr>"), |d| if d.extras.numerar_linhas == NumeracaoDeLinhas::PorPagina { p("por página, mas de 1 em 1 (o passo de 5 some)") } else { f(format!("{:?}", d.extras.numerar_linhas)) }, &["w:countBy=\"5\" w:restart=\"newPage\""]).parcial(&["w:restart=\"newPage\""]));

    s = "9. Cabeçalho e rodapé";
    let ref_cab = |extra: &str| sect(&format!("<w:headerReference w:type=\"default\" r:id=\"rIdCab\"/>{extra}"));
    v.push(it(s, "Cabeçalho com texto", pp("").replace(Z, "corpo") + &ref_cab(""), |d| se(faixas_com_z(&d.extras.cabecalho), format!("{:?}", d.extras.cabecalho.esquerda)), &["header>ZQX"]).partes(&[("word/header1.xml", cab(&par("", &r("", Z))))]));
    v.push(it(s, "Rodapé com número de página (PAGE)", pp("").replace(Z, "corpo") + &sect("<w:footerReference w:type=\"default\" r:id=\"rIdRod\"/>"), |d| se(d.extras.rodape.centro.contains("{pagina}"), format!("{:?}", d.extras.rodape)), &["footer>w:instr=\" PAGE \""]).partes(&[("word/footer1.xml", faixa("ftr", &format!("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:fldSimple w:instr=\" PAGE \"><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p>")))]));
    v.push(it(s, "\"Página X de Y\" (PAGE e NUMPAGES)", pp("").replace(Z, "corpo") + &sect("<w:footerReference w:type=\"default\" r:id=\"rIdRod\"/>"), |d| { let t = format!("{}{}{}", d.extras.rodape.esquerda, d.extras.rodape.centro, d.extras.rodape.direita); se(t.contains("Página {pagina} de {total}"), t) }, &["footer> PAGE ", "footer> NUMPAGES "]).partes(&[("word/footer1.xml", faixa("ftr", &format!("<w:p><w:pPr><w:jc w:val=\"right\"/></w:pPr>{}<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>{}<w:fldSimple w:instr=\" NUMPAGES \"><w:r><w:t>3</w:t></w:r></w:fldSimple></w:p>", r("", "Página "), r("", " de "))))]));
    v.push(it(s, "Três posições com tabulação (esquerda, centro, direita)", pp("").replace(Z, "corpo") + &ref_cab(""), |d| { let c = &d.extras.cabecalho; se(c.esquerda == "Empresa" && c.centro == Z && c.direita == "2026", format!("{:?} | {:?} | {:?}", c.esquerda, c.centro, c.direita)) }, &["header>Empresa", "header><w:tab/>", "header>2026"]).partes(&[("word/header1.xml", cab(&format!("<w:p><w:pPr><w:tabs><w:tab w:val=\"center\" w:pos=\"4513\"/><w:tab w:val=\"right\" w:pos=\"9026\"/></w:tabs></w:pPr>{}<w:r><w:tab/></w:r>{}<w:r><w:tab/></w:r>{}</w:p>", r("", "Empresa"), r("", Z), r("", "2026"))))]));
    v.push(it(s, "Letra do cabeçalho (negrito, tamanho, cor)", pp("").replace(Z, "corpo") + &ref_cab(""), |d| { let c = &d.extras.cabecalho; if !faixas_com_z(c) { f("sem cabeçalho") } else { { let ok = c.estilo & NEGRITO != 0 && c.tamanho == 14.0; se(ok, if ok { "negrito, 14 pt" } else { "o texto fica, a letra some" }) } } }, &["header><w:b/>"]).partes(&[("word/header1.xml", cab(&par("", &r("<w:b/><w:sz w:val=\"28\"/><w:color w:val=\"C00000\"/>", Z))))]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("corpo")] });
        d.extras.cabecalho = Faixas { esquerda: Z.into(), estilo: NEGRITO, tamanho: 14.0, cor: 0xC00000FF, ..Faixas::default() };
        d
    }));
    v.push(it(s, "Imagem no cabeçalho (logo)", pp("").replace(Z, "corpo") + &ref_cab(""), |d| { let ok = !d.extras.cabecalho.imagem.is_empty(); se(ok, if ok { "imagem" } else { "sem a imagem" }) }, &["header><wp:inline"]).partes(&[("word/header1.xml", cab(&img_par(&desenho(false, "", "", "")))), ("word/_rels/header1.xml.rels", RELS.to_string())]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("corpo")] });
        d.extras.cabecalho = Faixas { imagem: base64::para_data_url("image/png", PNG), imagem_largura: 60.0, imagem_altura: 30.0, ..Faixas::default() };
        d
    }));
    v.push(it(s, "Tabela no cabeçalho", pp("").replace(Z, "corpo") + &ref_cab(""), |d| if faixas_com_z(&d.extras.cabecalho) { p(format!("a tabela vira texto corrido: {:?}", d.extras.cabecalho.esquerda)) } else { f("some") }, &["header><w:tbl>"]).parcial(&["header>ZQX"]).partes(&[("word/header1.xml", cab(&(tabela("", "<w:gridCol w:w=\"4000\"/><w:gridCol w:w=\"4000\"/>", &[tr("", &[cel("Logo"), cel(Z)])]) + "<w:p/>")))]));
    v.push(it(s, "Primeira página diferente", pp("").replace(Z, "corpo") + &ref_cab("<w:headerReference w:type=\"first\" r:id=\"rIdCab2\"/><w:titlePg/>").replace("<w:titlePg/><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/>", "<w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/><w:titlePg/>"), |d| match (d.extras.primeira_diferente, faixas_com_z(&d.extras.cabecalho_primeira)) {
        (true, true) => x("primeira diferente, com o cabeçalho dela"),
        (true, false) => p("liga a primeira diferente, mas o cabeçalho dela vem vazio"),
        _ => f("não liga"),
    }, &["<w:titlePg/>", "w:type=\"first\""]).parcial(&["<w:titlePg/>"]).partes(&[("word/header1.xml", cab(&par("", &r("", "outras")))), ("word/header2.xml", cab(&par("", &r("", Z))))]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("corpo")] });
        d.extras.primeira_diferente = true;
        d.extras.cabecalho = Faixas { esquerda: "outras".into(), ..Faixas::default() };
        d.extras.cabecalho_primeira = Faixas { esquerda: Z.into(), ..Faixas::default() };
        d
    }));
    v.push(it(s, "Páginas pares e ímpares diferentes", pp("").replace(Z, "corpo") + &ref_cab("<w:headerReference w:type=\"even\" r:id=\"rIdCab3\"/>"), |d| se(faixas_com_z(&d.extras.cabecalho), "o modelo não tem cabeçalho de página par"), &["w:type=\"even\"", "settings><w:evenAndOddHeaders"]).partes(&[("word/header1.xml", cab(&par("", &r("", "ímpar")))), ("word/header3.xml", cab(&par("", &r("", Z)))), ("word/settings.xml", settings("<w:evenAndOddHeaders/>"))]));
    v.push(it(s, "Cabeçalho próprio em cada seção", pp(&sect("<w:headerReference w:type=\"default\" r:id=\"rIdCab\"/><w:type w:val=\"nextPage\"/>")) + &par("", &r("", "depois")) + &sect("<w:headerReference w:type=\"default\" r:id=\"rIdCab2\"/>"), |d| {
        let c = format!("{:?}", d.extras.cabecalho);
        match (c.contains(Z), c.contains("SEGUNDA")) {
            (true, true) => x("os dois"),
            (false, false) => f("nenhum"),
            _ => p(format!("só um cabeçalho no documento: {:?}", d.extras.cabecalho.esquerda)),
        }
    }, &["header>ZQX", "header>SEGUNDA"]).partes(&[("word/header1.xml", cab(&par("", &r("", Z)))), ("word/header2.xml", cab(&par("", &r("", "SEGUNDA"))))]));

    s = "10. Notas de rodapé e de fim";
    let notas_xml = |raiz: &str, tag: &str, corpo: &str| format!("<w:{raiz} {NS}><w:{tag} w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:{tag}><w:{tag} w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{tag}><w:{tag} w:id=\"1\">{corpo}</w:{tag}></w:{raiz}>");
    let chamada = |tag: &str| par("", &(r("", "texto") + &format!("<w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:{tag}Reference w:id=\"1\"/></w:r>")));
    v.push(it(s, "Nota de rodapé", chamada("footnote"), |d| se(notas(d).iter().any(|(fim, t)| !fim && texto_puro(t).contains(Z)), format!("{:?}", notas(d).iter().map(|(f, t)| (f, texto_puro(t))).collect::<Vec<_>>())), &["<w:footnoteReference", "footnotes>ZQX"]).partes(&[("word/footnotes.xml", notas_xml("footnotes", "footnote", &par("", &(format!("<w:r><w:footnoteRef/></w:r>") + &r("", " ") + &r("", Z)))))]));
    v.push(it(s, "Nota de fim", chamada("endnote"), |d| se(notas(d).iter().any(|(fim, t)| *fim && texto_puro(t).contains(Z)), format!("{:?}", notas(d).iter().map(|(f, t)| (f, texto_puro(t))).collect::<Vec<_>>())), &["<w:endnoteReference", "endnotes>ZQX"]).partes(&[("word/endnotes.xml", notas_xml("endnotes", "endnote", &par("", &(format!("<w:r><w:endnoteRef/></w:r>") + &r("", " ") + &r("", Z)))))]));
    v.push(it(s, "Formatação no texto da nota", chamada("footnote"), |d| se(notas(d).iter().any(|(_, t)| t.iter().any(|tr| tr.texto.contains(Z) && tr.estilo & ITALICO != 0)), "itálico na nota"), &["footnotes><w:i/>"]).partes(&[("word/footnotes.xml", notas_xml("footnotes", "footnote", &par("", &(format!("<w:r><w:footnoteRef/></w:r>") + &r("<w:i/>", Z)))))]));
    v.push(it(s, "Vários parágrafos numa nota", chamada("footnote"), |d| match notas(d).first() {
        Some((_, t)) if texto_puro(t).contains("um") && texto_puro(t).contains(Z) => p(format!("viram um parágrafo só: {:?}", texto_puro(t))),
        _ => f("texto perdido"),
    }, &["footnotes>um</w:t></w:r></w:p><w:p>"]).parcial(&["footnotes>ZQX"]).partes(&[("word/footnotes.xml", notas_xml("footnotes", "footnote", &(par("", &(format!("<w:r><w:footnoteRef/></w:r>") + &r("", "um"))) + &par("", &r("", Z)))))]));
    v.push(it(s, "Formato do número das notas (i, ii, iii)", chamada("footnote") + &sect("<w:footnotePr><w:numFmt w:val=\"lowerRoman\"/></w:footnotePr>"), |d| se(d.extras.formato_notas == 2, format!("formato {}", d.extras.formato_notas)), &["<w:footnotePr><w:numFmt w:val=\"lowerRoman\""]).partes(&[("word/footnotes.xml", notas_xml("footnotes", "footnote", &par("", &r("", Z))))]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("texto"), Trecho::objeto(Marca::Nota { fim: false, texto: vec![Trecho::simples(Z)] })] });
        d.extras.formato_notas = 2;
        d
    }));
    v.push(it(s, "Recomeçar a numeração das notas a cada página", chamada("footnote") + &sect("<w:footnotePr><w:numRestart w:val=\"eachPage\"/></w:footnotePr>"), |_| f("o modelo não tem"), &["<w:numRestart w:val=\"eachPage\""]).partes(&[("word/footnotes.xml", notas_xml("footnotes", "footnote", &par("", &r("", Z))))]));

    s = "11. Comentários";
    let comentado = par("", &(r("", "antes ") + "<w:commentRangeStart w:id=\"0\"/>" + &r("", "trecho") + "<w:commentRangeEnd w:id=\"0\"/><w:r><w:commentReference w:id=\"0\"/></w:r>"));
    let comentarios_xml = |corpo: &str| format!("<w:comments {NS}>{corpo}</w:comments>");
    let um_com = |id: u32, para: &str, texto: &str| format!("<w:comment w:id=\"{id}\" w:author=\"Ana Lima\" w:date=\"2026-10-28T10:00:00Z\" w:initials=\"AL\"><w:p w14:paraId=\"{para}\"><w:r><w:t>{texto}</w:t></w:r></w:p></w:comment>");
    v.push(it(s, "Comentário num trecho", comentado.clone(), |d| se(d.comentarios.iter().any(|c| c.texto.contains(Z)) && marcas(d).iter().filter(|m| matches!(m, Marca::Comentario { .. })).count() == 2, format!("{} comentários", d.comentarios.len())), &["<w:commentRangeStart", "<w:commentReference", "comments>ZQX"]).partes(&[("word/comments.xml", comentarios_xml(&um_com(0, "00000001", Z)))]));
    v.push(it(s, "Autor e data do comentário", comentado.clone(), |d| d.comentarios.first().map_or_else(|| f("sem comentário"), |c| se(c.autor == "Ana Lima" && c.quando == "28/10/2026", format!("{} {}", c.autor, c.quando))), &["comments>w:author=\"Ana Lima\"", "comments>w:date=\"2026-10-28"]).partes(&[("word/comments.xml", comentarios_xml(&um_com(0, "00000001", Z)))]));
    v.push(it(s, "Iniciais do autor", comentado.clone(), |d| d.comentarios.first().map_or_else(|| f("sem comentário"), |_| p("refeitas do nome")), &["comments>w:initials=\"AL\""]).partes(&[("word/comments.xml", comentarios_xml(&um_com(0, "00000001", Z)))]));
    v.push(it(s, "Resposta a comentário", comentado.replace("<w:commentRangeStart w:id=\"0\"/>", "<w:commentRangeStart w:id=\"0\"/><w:commentRangeStart w:id=\"1\"/>").replace("<w:commentRangeEnd w:id=\"0\"/>", "<w:commentRangeEnd w:id=\"0\"/><w:commentRangeEnd w:id=\"1\"/>"), |d| d.comentarios.first().map_or_else(|| f("sem comentário"), |c| se(c.respostas.iter().any(|r| r.texto.contains(Z)), format!("{} respostas", c.respostas.len()))), &["commentsExtended>w15:paraIdParent", "comments>ZQX"]).partes(&[("word/comments.xml", comentarios_xml(&(um_com(0, "00000001", "pergunta") + &um_com(1, "00000002", Z)))), ("word/commentsExtended.xml", format!("<w15:commentsEx {NS}><w15:commentEx w15:paraId=\"00000001\" w15:done=\"0\"/><w15:commentEx w15:paraId=\"00000002\" w15:paraIdParent=\"00000001\" w15:done=\"0\"/></w15:commentsEx>"))]));
    v.push(it(s, "Comentário resolvido", comentado.clone(), |d| d.comentarios.first().map_or_else(|| f("sem comentário"), |c| se(c.resolvido, format!("resolvido {}", c.resolvido))), &["commentsExtended>w15:done=\"1\""]).partes(&[("word/comments.xml", comentarios_xml(&um_com(0, "00000001", Z))), ("word/commentsExtended.xml", format!("<w15:commentsEx {NS}><w15:commentEx w15:paraId=\"00000001\" w15:done=\"1\"/></w15:commentsEx>"))]));
    v.push(it(s, "Formatação dentro do comentário", comentado.clone(), |d| d.comentarios.first().map_or_else(|| f("sem comentário"), |c| if c.texto.contains(Z) { p("o texto fica, o negrito some (o comentário é texto puro)") } else { f("texto perdido") }), &["comments><w:b/>"]).parcial(&["comments>ZQX"]).partes(&[("word/comments.xml", comentarios_xml(&um_com(0, "00000001", "x").replace("<w:r><w:t>x</w:t></w:r>", &r("<w:b/>", Z))))]));

    s = "12. Alterações controladas";
    v.push(it(s, "Inserção", par("", &(r("", "antes ") + "<w:ins w:id=\"1\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\">" + &r("", Z) + "</w:ins>")), |d| com_alvo(d, |t| se(t.rev.as_deref().is_some_and(|r| r.tipo == TipoDeRevisao::Insercao && r.autor == "Ana" && r.quando == "28/10/2026"), format!("{:?}", t.rev))), &["<w:ins w:id=", "w:author=\"Ana\""]));
    v.push(it(s, "Exclusão", par("", &(r("", "antes ") + "<w:del w:id=\"1\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\"><w:r><w:delText>ZQX</w:delText></w:r></w:del>")), |d| com_alvo(d, |t| se(t.rev.as_deref().is_some_and(|r| r.tipo == TipoDeRevisao::Remocao && r.autor == "Ana"), format!("{:?}", t.rev))), &["<w:del w:id=", "<w:delText"]));
    v.push(it(s, "Formatação alterada (rPrChange)", pr("<w:b/><w:rPrChange w:id=\"2\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\"><w:rPr/></w:rPrChange>"), |d| com_alvo(d, |t| if t.estilo & NEGRITO != 0 { f("fica a formatação nova, sem a marca de alteração") } else { f("") }), &["<w:rPrChange"]));
    v.push(it(s, "Parágrafo alterado (pPrChange)", pp("<w:jc w:val=\"center\"/><w:pPrChange w:id=\"3\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\"><w:pPr/></w:pPrChange>"), |d| com_bloco(d, |_| f("fica o parágrafo novo, sem a marca de alteração")), &["<w:pPrChange"]));
    v.push(it(s, "Texto movido (moveFrom/moveTo)", par("", &(r("", "um ") + "<w:moveFrom w:id=\"4\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\">" + &r("", Z) + "</w:moveFrom>")) + &par("", &("<w:moveTo w:id=\"5\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\">".to_string() + &r("", Z) + "</w:moveTo>")), |d| {
        let n = quantos_z(d);
        let marcados = trechos(d).iter().filter(|t| t.texto.contains(Z) && t.rev.is_some()).count();
        if marcados == 2 { x("de onde saiu e para onde foi") } else { f(format!("o texto aparece {n} vezes, {marcados} com marca")) }
    }, &["<w:moveFrom", "<w:moveTo"]).parcial(&["<w:del ", "<w:ins "]));
    v.push(it(s, "Marca de parágrafo inserido", pp("<w:rPr><w:ins w:id=\"6\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\"/></w:rPr>"), |d| com_alvo(d, |t| se(t.rev.is_some(), "o fim de parágrafo inserido não tem marca no modelo")), &["<w:rPr><w:ins "]));
    v.push(it(s, "Controlar alterações ligado (trackRevisions)", pp(""), |d| se(d.extras.controlar_alteracoes, format!("{}", d.extras.controlar_alteracoes)), &["settings><w:trackRevisions"]).partes(&[("word/settings.xml", settings("<w:trackRevisions/>"))]));
    v.push(it(s, "Linha de tabela inserida", tabela2("", "<w:ins w:id=\"7\" w:author=\"Ana\" w:date=\"2026-10-28T10:00:00Z\"/>", ""), |d| tabela_de(d).map_or_else(|| f("sem tabela"), |_| f("a linha fica, sem a marca de alteração")), &["<w:trPr><w:ins "]));

    s = "13. Campos";
    v.push(it(s, "PAGE no corpo (campo complexo)", campo_complexo("PAGE", "1"), |d| se(campo(d).as_deref() == Some("pagina"), format!("{:?}", campo(d))), &["w:instr=\" PAGE \""]).parcial(&["PAGE"]));
    v.push(it(s, "NUMPAGES (campo simples)", campo_simples("NUMPAGES", "3"), |d| se(campo(d).as_deref() == Some("paginas"), format!("{:?}", campo(d))), &["NUMPAGES"]));
    v.push(it(s, "DATE com formato", campo_simples("DATE \\@ \"d 'de' MMMM 'de' yyyy\"", "28 de outubro de 2026"), |d| if campo(d).as_deref() == Some("data") { p("fica o campo; o formato vira dd/MM/yyyy") } else { f(format!("{:?}", campo(d))) }, &["DATE \\@ &quot;d 'de' MMMM"]).parcial(&["DATE"]));
    v.push(it(s, "AUTHOR e TITLE", campo_simples("AUTHOR", "Ana"), |d| se(campo(d).as_deref() == Some("autor"), format!("{:?}", campo(d))), &["AUTHOR"]));
    v.push(it(s, "FILENAME", campo_simples("FILENAME", "a.docx"), |d| se(campo(d).as_deref() == Some("arquivo"), format!("{:?}", campo(d))), &["FILENAME"]));
    v.push(it(s, "HYPERLINK como campo", campo_complexo("HYPERLINK \"http://exemplo.org/\"", Z), |d| com_alvo(d, |t| if t.link.as_deref() == Some("http://exemplo.org/") { x("link") } else { p("fica o texto, sem o link") }), &["<w:hyperlink"]));
    v.push(it(s, "MERGEFIELD (mala direta)", campo_complexo("MERGEFIELD Nome", "«Nome»"), |d| if texto_todo(d).contains("«Nome»") { p("fica o texto «Nome», sem o campo") } else { f("some") }, &["MERGEFIELD"]).parcial(&["«Nome»"]));
    v.push(it(s, "Campo de cálculo (= SUM(ABOVE), IF)", campo_complexo("= 2+3", "5"), |d| if texto_todo(d).contains('5') { p("fica o resultado como texto") } else { f("some") }, &["= 2+3"]).parcial(&[">5</w:t>"]));
    v.push(it(s, "Resultado do campo guardado (campo desconhecido)", campo_complexo("DOCPROPERTY Empresa", Z), |d| if alvo(d).is_some() { p("fica o texto do resultado, sem o campo") } else { sumiu() }, &["DOCPROPERTY"]).parcial(&["ZQX"]));

    s = "14. Sumário, legendas, indicadores e links";
    let toc = |instr: &str| format!("<w:p><w:pPr><w:pStyle w:val=\"CabealhodoSumrio\"/></w:pPr>{}</w:p><w:p><w:pPr><w:pStyle w:val=\"Sumrio1\"/></w:pPr><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>{}<w:r><w:tab/></w:r>{}</w:p><w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>{}", r("", "Sumário"), r("", "Introdução"), r("", "1"), pp("<w:pStyle w:val=\"Ttulo1\"/>").replace(Z, "Introdução"));
    v.push(it(s, "Sumário (campo TOC)", toc("TOC \\o \"1-3\" \\h \\z \\u"), |d| { let s = sumario(d); se(s.is_some_and(|s| s.0 == 3) && !texto_todo(d).contains("Sumário"), format!("{s:?}; título duplicado {}", texto_todo(d).contains("Sumário"))) }, &["TOC \\o &quot;1-3&quot;"]).parcial(&["TOC"]));
    v.push(it(s, "Sumário com outros níveis e sem números de página", toc("TOC \\o \"1-2\" \\n \\h"), |d| { let s = sumario(d); se(s.is_some_and(|s| s.0 == 2 && s.1), format!("{s:?}")) }, &["TOC \\o &quot;1-2&quot;", "\\n"]));
    v.push(it(s, "Entradas do sumário guardadas (texto e página)", toc("TOC \\o \"1-3\""), |d| if sumario(d).is_some() { p("o sumário é refeito dos títulos; as entradas guardadas pelo Word somem") } else { f("sem sumário") }, &["<w:pStyle w:val=\"TOC1\""]).parcial(&["TOC"]));
    v.push(it(s, "Lista de figuras (TOC \\c)", toc("TOC \\h \\z \\c \"Figura\""), |d| { let s = sumario(d); se(s.is_some_and(|s| s.2 == 1), format!("{s:?}")) }, &["\\c &quot;Figura&quot;"]));
    let legenda = |rotulo: &str| par("<w:pStyle w:val=\"Legenda\"/>", &(r("", &format!("{rotulo} ")) + &format!("<w:fldSimple w:instr=\" SEQ {rotulo} \\* ARABIC \"><w:r><w:t>1</w:t></w:r></w:fldSimple>") + &r("", " – ") + &r("", Z)));
    v.push(it(s, "Legenda de figura (SEQ Figura)", legenda("Figura"), |d| com_bloco(d, |b| se(b.atrib.legenda == 1 && !texto_puro(b.conteudo.textos()[0]).contains("Figura"), format!("legenda {} {:?}", b.atrib.legenda, texto_puro(b.conteudo.textos()[0])))), &["SEQ Figura"]));
    v.push(it(s, "Legenda de tabela (SEQ Tabela)", legenda("Tabela"), |d| com_bloco(d, |b| se(b.atrib.legenda == 2, format!("legenda {}", b.atrib.legenda))), &["SEQ Tabela"]));
    v.push(it(s, "Legenda com rótulo próprio (SEQ Quadro)", legenda("Quadro"), |d| com_bloco(d, |b| if b.atrib.legenda > 0 { p(format!("vira legenda de {}", ["", "figura", "tabela", "equação"][usize::from(b.atrib.legenda.min(3))])) } else { f("") }), &["SEQ Quadro"]).parcial(&["SEQ "]));
    v.push(it(s, "Indicador (bookmark)", "<w:p><w:bookmarkStart w:id=\"0\" w:name=\"sec1\"/>".to_string() + &r("", Z) + "<w:bookmarkEnd w:id=\"0\"/></w:p>", |d| se(d.marcadores.iter().any(|m| m.nome == "sec1"), format!("{} marcadores", d.marcadores.len())), &["w:bookmarkStart", "w:name=\"sec1\""]).montar(|| {
        let mut d = Documento::vazio();
        let id = d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples(Z)] });
        d.marcadores.push(Marcador { nome: "sec1".into(), bloco: id });
        d
    }));
    v.push(it(s, "Referência cruzada (REF)", "<w:p><w:bookmarkStart w:id=\"0\" w:name=\"sec1\"/>".to_string() + &r("", "Alvo") + "<w:bookmarkEnd w:id=\"0\"/></w:p>" + &campo_complexo("REF sec1 \\h", Z), |d| if alvo(d).is_some_and(|t| t.link.as_deref() == Some("#sec1")) { p("vira link para o indicador") } else if alvo(d).is_some() { p("fica o texto do resultado, sem a referência") } else { sumiu() }, &["REF sec1"]).parcial(&["ZQX"]));
    v.push(it(s, "Referência à página (PAGEREF)", "<w:p><w:bookmarkStart w:id=\"0\" w:name=\"sec1\"/>".to_string() + &r("", "Alvo") + "<w:bookmarkEnd w:id=\"0\"/></w:p>" + &campo_complexo("PAGEREF sec1 \\h", Z), |d| if alvo(d).is_some() { p("fica o número como texto, sem a referência") } else { sumiu() }, &["PAGEREF sec1"]).parcial(&["ZQX"]));
    v.push(it(s, "Hiperlink externo", "<w:p><w:hyperlink r:id=\"rIdLink\">".to_string() + &r("<w:rStyle w:val=\"Hyperlink\"/>", Z) + "</w:hyperlink></w:p>", |d| com_alvo(d, |t| se(t.link.as_deref() == Some("http://exemplo.org/"), format!("{:?}", t.link))), &["<w:hyperlink r:id=", "rels>http://exemplo.org/"]));
    v.push(it(s, "Hiperlink interno (para indicador)", "<w:p><w:hyperlink w:anchor=\"sec1\">".to_string() + &r("", Z) + "</w:hyperlink></w:p>", |d| com_alvo(d, |t| se(t.link.as_deref() == Some("#sec1"), format!("{:?}", t.link))), &["w:anchor=\"sec1\""]));
    v.push(it(s, "Dica de tela do link (tooltip)", "<w:p><w:hyperlink r:id=\"rIdLink\" w:tooltip=\"Abrir o site\">".to_string() + &r("", Z) + "</w:hyperlink></w:p>", |d| com_alvo(d, |t| if t.link.is_some() { p("fica o link, a dica some") } else { f("") }), &["w:tooltip=\"Abrir o site\""]).parcial(&["<w:hyperlink"]));
    v.push(it(s, "Índice remissivo (XE e INDEX)", par("", &(r("", Z) + "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> XE \"termo\" </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>")) + &campo_complexo("INDEX \\e \"\t\"", "termo 1"), |d| {
        let xe = marcas(d).iter().any(|m| matches!(m, Marca::Indice { termo, .. } if termo == "termo"));
        let idx = d.blocos.iter().any(|b| matches!(b.conteudo, Conteudo::IndiceRemissivo));
        match (xe, idx) { (true, true) => x("entrada e índice"), (false, false) => f("nada"), _ => p(format!("entrada {xe}, índice {idx}")) }
    }, &["XE &quot;termo&quot;", "INDEX"]));
    v.push(it(s, "Citação e fontes (CITATION e b:Sources)", campo_simples("CITATION silva2023 \\l 1046", "(SILVA, 2023)"), |d| {
        let cit = marcas(d).iter().any(|m| matches!(m, Marca::Citacao { chaves, .. } if chaves.iter().any(|c| c == "silva2023")));
        let fonte = d.bibliografia.fontes.iter().any(|f| f.chave == "silva2023" && f.titulo == "Redes");
        match (cit, fonte) { (true, true) => x("citação e fonte"), (false, false) => f("nada"), _ => p(format!("citação {cit}, fonte {fonte}")) }
    }, &["CITATION silva2023", "customXml/item1>silva2023"]).partes(&[("customXml/item1.xml", "<b:Sources xmlns:b=\"http://schemas.openxmlformats.org/officeDocument/2006/bibliography\" StyleName=\"APA\"><b:Source><b:Tag>silva2023</b:Tag><b:SourceType>Book</b:SourceType><b:Title>Redes</b:Title><b:Year>2023</b:Year></b:Source></b:Sources>".to_string())]));

    s = "15. Fórmulas (OMML)";
    let fracao = "<m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f>";
    v.push(it(s, "Fórmula em linha", par("", &(r("", "antes ") + &format!("<m:oMath>{fracao}</m:oMath>") + &r("", " ZQX"))), |d| { let m = trechos(d).into_iter().find(|t| t.estilo & MATEMATICA != 0); se(m.is_some_and(|t| t.texto.contains("\\frac")), format!("{:?}", m.map(|t| &t.texto))) }, &["<m:oMath><m:f>", "<m:num>"]).parcial(&["<m:oMath>"]));
    v.push(it(s, "Fórmula em destaque (oMathPara)", format!("<w:p><m:oMathPara><m:oMath>{fracao}</m:oMath></m:oMathPara></w:p>"), |d| { let e = d.blocos.iter().find_map(|b| if let Conteudo::Equacao { latex } = &b.conteudo { Some(latex.clone()) } else { None }); se(e.as_deref().is_some_and(|l| l.contains("\\frac")), format!("{e:?}")) }, &["<m:oMathPara>", "<m:f>"]));
    v.push(it(s, "Matriz, somatório e raiz", "<w:p><m:oMathPara><m:oMath><m:nary><m:naryPr><m:chr m:val=\"∑\"/></m:naryPr><m:sub><m:r><m:t>i</m:t></m:r></m:sub><m:sup><m:r><m:t>n</m:t></m:r></m:sup><m:e><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:e></m:nary><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr></m:m></m:oMath></m:oMathPara></w:p>".to_string(), |d| { let e = d.blocos.iter().find_map(|b| if let Conteudo::Equacao { latex } = &b.conteudo { Some(latex.clone()) } else { None }); let l = e.clone().unwrap_or_default(); let ok = [l.contains("\\sum"), l.contains("\\sqrt"), l.contains("matrix")]; if ok.iter().all(|b| *b) { x(l) } else if ok.iter().any(|b| *b) { p(format!("só parte: {l}")) } else { f(l) } }, &["<m:nary>", "<m:rad>", "<m:m>"]));
    v.push(it(s, "Fórmula numerada (equação com SEQ)", par("<w:pStyle w:val=\"Legenda\"/>", &(format!("<m:oMath>{fracao}</m:oMath>") + &r("", " (") + "<w:fldSimple w:instr=\" SEQ Equação \\* ARABIC \"><w:r><w:t>1</w:t></w:r></w:fldSimple>" + &r("", ")"))), |d| if d.blocos.iter().any(|b| b.atrib.legenda == 3) { if trechos(d).iter().any(|t| t.estilo & MATEMATICA != 0) { x("legenda de equação com a fórmula") } else { p("vira legenda de equação, mas a fórmula antes do SEQ some") } } else { f(format!("{:?}", d.blocos.iter().map(tipo).collect::<Vec<_>>())) }, &["SEQ Equação", "<m:oMath>"]).parcial(&["SEQ Equação"]));

    s = "16. Documento, idioma e marca d'água";
    v.push(it(s, "Título e autor (docProps/core.xml)", pp(""), |d| se(d.meta.titulo == "Relatório" && d.meta.autor == "Ana", format!("{:?} {:?}", d.meta.titulo, d.meta.autor)), &["core><dc:title>Relatório", "core><dc:creator>Ana"]).partes(&[("docProps/core.xml", "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>Relatório</dc:title><dc:creator>Ana</dc:creator><dc:subject>Vendas</dc:subject><cp:keywords>anual</cp:keywords></cp:coreProperties>".to_string())]));
    v.push(it(s, "Assunto, palavras-chave, descrição, datas (core.xml)", pp(""), |_| f("o modelo só tem título e autor"), &["core><dc:subject>Vendas", "core><cp:keywords>anual"]).partes(&[("docProps/core.xml", "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>Relatório</dc:title><dc:subject>Vendas</dc:subject><cp:keywords>anual</cp:keywords></cp:coreProperties>".to_string())]));
    v.push(it(s, "Propriedades do aplicativo (app.xml: empresa, gerente)", pp(""), |_| f("o modelo não tem"), &["app.xml><Company>"]).partes(&[("docProps/app.xml", "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Company>Feather</Company><Manager>Bia</Manager></Properties>".to_string())]));
    v.push(it(s, "Propriedades personalizadas (custom.xml)", pp(""), |_| f("o modelo não tem"), &["custom.xml>Projeto"]).partes(&[("docProps/custom.xml", "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><property fmtid=\"{D5CDD505-2E9C-101B-9397-08002B2CF9AE}\" pid=\"2\" name=\"Projeto\"><vt:lpwstr>Pena</vt:lpwstr></property></Properties>".to_string())]));
    v.push(it(s, "Marca d'água de texto", pp("").replace(Z, "corpo") + &ref_cab(""), |d| se(d.extras.marca == "CONFIDENCIAL", format!("{:?}", d.extras.marca)), &["header>CONFIDENCIAL"]).partes(&[("word/header1.xml", cab("<w:p><w:r><w:pict><v:shape id=\"PowerPlusWaterMarkObject\" type=\"#_x0000_t136\" style=\"rotation:315;width:400pt;height:100pt\" fillcolor=\"silver\"><v:textpath string=\"CONFIDENCIAL\"/></v:shape></w:pict></w:r></w:p>"))]).montar(|| {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![Trecho::simples("corpo")] });
        d.extras.marca = "CONFIDENCIAL".into();
        d.extras.marca_cor = 0xC0C0C0FF;
        d
    }));
    v.push(it(s, "Proteção do documento (só leitura)", pp(""), |d| se(d.extras.protegido, format!("{}", d.extras.protegido)), &["settings><w:documentProtection w:edit=\"readOnly\""]).partes(&[("word/settings.xml", settings("<w:documentProtection w:edit=\"readOnly\" w:enforcement=\"1\"/>"))]));
    v.push(it(s, "Fontes embutidas (embedTrueTypeFonts)", pp(""), |_| f("o modelo não tem"), &["settings><w:embedTrueTypeFonts", "fontTable>w:embedRegular"]).partes(&[("word/settings.xml", settings("<w:embedTrueTypeFonts/>")), ("word/fontTable.xml", format!("<w:fonts {NS}><w:font w:name=\"Minha Fonte\"><w:embedRegular r:id=\"rIdF1\" w:fontKey=\"{{00000000-0000-0000-0000-000000000000}}\"/></w:font></w:fonts>")), ("word/fonts/font1.odttf", "x".to_string())]));
    v.push(it(s, "Idioma do texto (w:lang)", pr("<w:lang w:val=\"en-US\"/>"), |d| com_alvo(d, |_| f("o modelo não tem idioma por trecho")), &["<w:lang w:val=\"en-US\""]));
    v.push(it(s, "Idioma padrão do documento", pp(""), |_| f("o modelo não tem; grava pt-BR fixo"), &["styles><w:lang w:val=\"es-ES\""]).partes(&[("word/styles.xml", estilos_padrao().replacen("<w:style ", "<w:docDefaults><w:rPrDefault><w:rPr><w:lang w:val=\"es-ES\"/></w:rPr></w:rPrDefault></w:docDefaults><w:style ", 1))]));
    v.push(it(s, "Texto da direita para a esquerda (bidi/rtl)", par("<w:bidi/>", &r("<w:rtl/>", Z)), |d| com_bloco(d, |b| se(b.atrib.alinhamento == Alinhamento::Direita, "o modelo não tem direção do texto")), &["<w:bidi/>", "<w:rtl/>"]));
    v.push(it(s, "Hifenização automática", pp(""), |_| f("o modelo não tem"), &["settings><w:autoHyphenation"]).partes(&[("word/settings.xml", settings("<w:autoHyphenation/>"))]));
    v.push(it(s, "Não hifenizar o parágrafo", pp("<w:suppressAutoHyphens/>"), |d| com_bloco(d, |_| f("o modelo não tem")), &["<w:suppressAutoHyphens"]));
    v.push(it(s, "Parada de tabulação padrão (defaultTabStop)", pp(""), |_| f("o modelo não tem; grava 708 fixo"), &["settings><w:defaultTabStop w:val=\"1134\""]).partes(&[("word/settings.xml", settings("<w:defaultTabStop w:val=\"1134\"/>"))]));
    v
}

// ---------------------------------------------------------------------------
// Medir
// ---------------------------------------------------------------------------

pub(crate) struct Resultado {
    pub(crate) le: Medida,
    pub(crate) grava: Medida,
    pub(crate) volta: Medida,
}

fn medir_grava(item: &Item, doc: &Documento) -> Medida {
    let Ok(bytes) = docx::gravar(doc) else { return f("o gravador falhou") };
    let partes = partes_de(&bytes);
    let tem = |padrao: &str| {
        if let Some(ausente) = padrao.strip_prefix('!') {
            let (parte, pad) = ausente.split_once('>').filter(|(p, _)| !p.contains('<') && !p.contains(' ')).unwrap_or(("document.xml", ausente));
            return !partes.iter().any(|(n, x)| n.contains(parte) && x.contains(pad));
        }
        let (parte, pad) = padrao.split_once('>').filter(|(p, _)| !p.contains('<') && !p.contains(' ')).unwrap_or(("document.xml", padrao));
        partes.iter().any(|(n, x)| n.contains(parte) && x.contains(pad))
    };
    let faltam: Vec<&str> = item.grava.iter().copied().filter(|p| !tem(p)).collect();
    if faltam.is_empty() {
        x("")
    } else if !item.parcial.is_empty() && item.parcial.iter().all(|p| tem(p)) || faltam.len() < item.grava.len() {
        p(format!("falta {}", faltam.join(" · ")))
    } else {
        f(format!("falta {}", faltam.join(" · ")))
    }
}

fn medir(item: &Item) -> Resultado {
    let bytes = montar_docx(item);
    let lido = match docx::ler(&bytes) {
        Ok(d) => d,
        Err(e) => return Resultado { le: f(format!("não abriu: {e}")), grava: f(""), volta: f("") },
    };
    let le = (item.le)(&lido);
    let montado = item.montar.map(|m| m());
    let grava = medir_grava(item, montado.as_ref().unwrap_or(&lido));
    let base = if le.0 != ' ' { Some(&lido) } else { montado.as_ref() };
    let volta = match base {
        None => f("nada para voltar"),
        Some(base) => {
            let r1 = (item.le)(base);
            let m2 = fdoz::gravar(base).and_then(|b| fdoz::ler(&b));
            let m3 = m2.as_ref().map_err(String::clone).and_then(|m| docx::gravar(m).and_then(|b| docx::ler(&b)));
            match (m2, m3) {
                (Err(e), _) => f(format!("o .fdoz falhou: {e}")),
                (_, Err(e)) => f(format!("o .docx falhou: {e}")),
                (Ok(m2), Ok(m3)) => {
                    let (r2, r3) = ((item.le)(&m2), (item.le)(&m3));
                    if r1.0 == ' ' {
                        f("o modelo montado não tem o recurso")
                    } else if r3 == r1 {
                        x("")
                    } else {
                        let onde = if r2 != r1 { "no .fdoz" } else { "no .docx" };
                        let c = if r3.0 != ' ' { '~' } else { ' ' };
                        (c, format!("perde {onde}: «{}» → «{}»", r1.1, r3.1))
                    }
                }
            }
        }
    };
    Resultado { le, grava, volta }
}

pub(crate) fn medir_protegido(item: &Item) -> Resultado {
    catch_unwind(AssertUnwindSafe(|| medir(item))).unwrap_or_else(|_| Resultado { le: f("pânico"), grava: f("pânico"), volta: f("pânico") })
}

/// A ordem de importância para o documento comum de trabalho (as 10 primeiras que não estão
/// `[x]` nas três colunas viram a lista de lacunas no fim).
const PRIORIDADE: &[&str] = &[
    "Estilo de parágrafo personalizado (formatação do estilo)",
    "Padrão do documento (docDefaults: fonte e tamanho)",
    "Larguras das colunas",
    "Mescla horizontal (gridSpan)",
    "Imagem flutuante (âncora, posição, quebra automática)",
    "Primeira página diferente",
    "Numeração vinda do estilo de parágrafo (List Number)",
    "Fonte (família)",
    "Mescla vertical (vMerge)",
    "Cabeçalho próprio em cada seção",
    "Várias seções com papel próprio (retrato e paisagem)",
    "Sombreamento de célula",
    "Bordas por célula (tcBorders)",
    "Herança de estilos (basedOn)",
    "Aparência do estilo de título (cor e tamanho do styles.xml)",
    "Início da numeração em outro número",
    "Imagem no cabeçalho (logo)",
    "Manter com o próximo",
    "Entrelinha exata",
    "Alinhamento do texto na célula",
    "Estilo de caractere (rStyle)",
    "Texto alternativo (descr)",
    "Imagem dentro de célula",
    "Colunas de texto",
    "Indicador (bookmark)",
    "Referência cruzada (REF)",
    "Caixa de texto",
    "Páginas pares e ímpares diferentes",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let saida = args.iter().position(|a| a == "--saida").and_then(|i| args.get(i + 1)).cloned();
    let detalhe = args.iter().any(|a| a == "--detalhe");
    let itens = itens();
    let resultados: Vec<Resultado> = itens.iter().map(medir_protegido).collect();

    let mut secoes: Vec<&str> = Vec::new();
    for i in &itens {
        if !secoes.contains(&i.secao) {
            secoes.push(i.secao);
        }
    }
    let conta = |sel: &dyn Fn(&Resultado) -> char, filtro: &dyn Fn(&Item) -> bool| {
        let mut c = [0usize; 3];
        for (i, r) in itens.iter().zip(&resultados) {
            if filtro(i) {
                c[match sel(r) { 'x' => 0, '~' => 1, _ => 2 }] += 1;
            }
        }
        c
    };
    let cols: [(&str, &dyn Fn(&Resultado) -> char); 3] = [("Lê", &|r| r.le.0), ("Grava", &|r| r.grava.0), ("Volta igual", &|r| r.volta.0)];

    let mut md = String::new();
    md.push_str("# Paridade do `.docx` (ida e volta `.fdoz` ↔ `.docx`)\n\n");
    md.push_str("> Levantamento de **28/10/2026**, status **MEDIDO**, não deduzido. Gerado por `crates/featherdocs-io/examples/paridade_docx.rs`; não editar à mão.\n\n");
    md.push_str("Cada item é um `.docx` mínimo montado no exemplo (XML à mão dentro de um ZIP), com o texto `ZQX` no lugar do recurso. As três medidas saem do modelo (`featherdocs-core/src/modelo.rs`), não de ler o código:\n\n");
    md.push_str("- **Lê** — `docx::ler` põe o efeito no modelo?\n");
    md.push_str("- **Grava** — o modelo vai para `docx::gravar` e o elemento aparece no XML de saída? O modelo é o lido; quando o leitor não preenche um campo que o modelo tem, o exemplo monta o modelo direto (itens com † na tabela).\n");
    md.push_str("- **Volta igual** — `.docx` → modelo → `.fdoz` (gravar e ler) → `.docx` → modelo dá o mesmo resultado para aquele recurso? (Com † e Lê `[ ]`, parte do modelo montado.) A volta compara com a primeira leitura: um `[~]` em Lê pode ter `[x]` na volta (o que entrou no modelo não se perde mais).\n\n");
    md.push_str("Legenda: `[x]` completo · `[~]` parcial (a observação diz o que perde) · `[ ]` falta.\n\n");
    md.push_str("**Como refazer** (a partir de `crates/featherdocs-io`, sob limite de memória):\n\n");
    md.push_str("```bash\nsystemd-run --user --scope -q -p MemoryMax=8G cargo run --example paridade_docx -j 6 -- --saida ../../docs/PARIDADE-DOCX.md\n```\n\n");
    md.push_str("`--detalhe` acrescenta o que cada medida viu. Para melhorar um item: mexa no leitor/gravador/modelo, rode de novo e confira a marca. Se o critério de um item estiver fraco, acerte-o no exemplo (a função `le` e os padrões `grava`).\n\n");

    md.push_str("## Resumo por seção\n\n| Seção | Itens | Lê (x / ~ / falta) | Grava (x / ~ / falta) | Volta (x / ~ / falta) |\n|---|---|---|---|---|\n");
    for sec in &secoes {
        let n = itens.iter().filter(|i| i.secao == *sec).count();
        let _ = write!(md, "| {sec} | {n} |");
        for (_, sel) in &cols {
            let c = conta(*sel, &|i: &Item| i.secao == *sec);
            let _ = write!(md, " {} / {} / {} |", c[0], c[1], c[2]);
        }
        md.push('\n');
    }
    let _ = write!(md, "| **Total** | **{}** |", itens.len());
    for (_, sel) in &cols {
        let c = conta(*sel, &|_| true);
        let _ = write!(md, " **{} / {} / {}** |", c[0], c[1], c[2]);
    }
    md.push_str("\n\n");

    let marca = |c: char| format!("[{c}]");
    for sec in &secoes {
        let _ = writeln!(md, "## {sec}\n\n| Recurso | Lê | Grava | Volta igual | Observação |\n|---|---|---|---|---|");
        for (i, r) in itens.iter().zip(&resultados).filter(|(i, _)| i.secao == *sec) {
            let mut obs: Vec<String> = Vec::new();
            if r.le.0 != 'x' && !r.le.1.is_empty() {
                obs.push(format!("Lê: {}", r.le.1));
            }
            if r.grava.0 != 'x' && !r.grava.1.is_empty() && (detalhe || r.le.0 != ' ' || i.montar.is_some()) {
                obs.push(format!("Grava: {}", r.grava.1.replace('|', "\\|")));
            }
            if r.volta.0 != 'x' && (r.le.0 != ' ' || i.montar.is_some()) {
                obs.push(format!("Volta: {}", r.volta.1));
            }
            if detalhe && r.le.0 == 'x' {
                obs.push(format!("viu: {}", r.le.1));
            }
            let nome = if i.montar.is_some() { format!("{} †", i.nome) } else { i.nome.to_string() };
            let obs = obs.join(" · ").replace('|', "\\|").replace('\n', " ");
            let _ = writeln!(md, "| {nome} | {} | {} | {} | {obs} |", marca(r.le.0), marca(r.grava.0), marca(r.volta.0));
        }
        md.push('\n');
    }

    md.push_str("## Resumo\n\n");
    let _ = writeln!(md, "**{} itens** em {} seções.\n", itens.len(), secoes.len());
    md.push_str("| Coluna | Completo `[x]` | Parcial `[~]` | Falta `[ ]` |\n|---|---|---|---|\n");
    for (nome, sel) in &cols {
        let c = conta(*sel, &|_| true);
        let _ = writeln!(md, "| {nome} | {} | {} | {} |", c[0], c[1], c[2]);
    }
    let completos = resultados.iter().filter(|r| r.le.0 == 'x' && r.grava.0 == 'x' && r.volta.0 == 'x').count();
    let _ = writeln!(md, "\nCompletos nas três colunas: **{completos} / {}**.\n", itens.len());

    md.push_str("### As 10 lacunas mais importantes para o documento comum de trabalho\n\n");
    md.push_str("Em ordem de uso no dia a dia (estilos, tabelas, imagens, listas, cabeçalho/rodapé, seções, sumário); a ordem é a constante `PRIORIDADE` do exemplo, e entram as 10 primeiras que não estão completas nas três colunas.\n\n");
    let mut n = 0;
    for nome in PRIORIDADE {
        let Some((i, r)) = itens.iter().zip(&resultados).find(|(i, _)| i.nome == *nome) else { continue };
        if r.le.0 == 'x' && r.grava.0 == 'x' && r.volta.0 == 'x' {
            continue;
        }
        n += 1;
        let porque = if r.le.0 != 'x' { r.le.1.clone() } else if r.grava.0 != 'x' { r.grava.1.clone() } else { r.volta.1.clone() };
        let _ = writeln!(md, "{n}. **{}** ({}) — Lê {} · Grava {} · Volta {} — {}", i.nome, i.secao, marca(r.le.0), marca(r.grava.0), marca(r.volta.0), porque);
        if n == 10 {
            break;
        }
    }
    md.push_str("\n## O que não está medido\n\n");
    md.push_str("- **Fidelidade visual** (quebras de linha e de página iguais às do Word, métricas de fonte): aqui só se mede se o recurso chega ao modelo e ao XML.\n");
    md.push_str("- **Arquivos reais**: os `.docx` são mínimos, feitos à mão; um documento do Word traz combinações (estilo + formatação direta + tema) que estes itens não cobrem. O próximo passo é rodar o mesmo critério sobre arquivos de usuário.\n");
    md.push_str("- **Tema** (`theme1.xml`): o leitor guarda o tema inteiro e o devolve, mas só entende as fontes (item \"Fonte de tema\"); as cores de tema não têm item que confira o vínculo além de \"Cor de tema\".\n");
    md.push_str("- **WordArt**, **tela de desenho e grupos de formas**, **vídeo online**, **macros** (`.docm`) e **assinatura digital**: sem item próprio; o leitor não abre essas partes.\n");
    md.push_str("- **Grava** só confere o elemento no XML, não se o Word o abre sem reclamar (o esquema completo do OOXML não é validado).\n");

    match saida {
        Some(caminho) => {
            std::fs::write(&caminho, &md).expect("gravar a saída");
            let c = [conta(&|r| r.le.0, &|_| true), conta(&|r| r.grava.0, &|_| true), conta(&|r| r.volta.0, &|_| true)];
            println!("{} itens · Lê {:?} · Grava {:?} · Volta {:?} · gravado em {caminho}", itens.len(), c[0], c[1], c[2]);
        }
        None => print!("{md}"),
    }
}
