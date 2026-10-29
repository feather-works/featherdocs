//! O que um objeto do Word (forma, gráfico, SmartArt, objeto OLE) é, lido do XML e das partes que
//! `Marca::ObjetoWord` guarda. O XML continua sendo o que vai e volta ao `.docx`; aqui só se entende o que ele diz.

use crate::modelo::ParteWord;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TipoDeObjeto {
    #[default]
    Outro,
    /// Forma do DrawingML (`wps:wsp`) ou da VML (`v:rect`, `v:shape` sem imagem).
    Forma,
    CaixaDeTexto,
    /// Gráfico (`c:chart`).
    Grafico,
    /// SmartArt (`dgm:relIds`).
    SmartArt,
    /// Objeto OLE (`o:OLEObject`): planilha, apresentação… incorporada ou vinculada.
    Ole,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoObjeto {
    pub tipo: TipoDeObjeto,
    /// O nome dado no Word (`wp:docPr name`).
    pub nome: String,
    /// O tamanho em pt (`wp:extent`, ou o estilo da VML).
    pub largura: f32,
    pub altura: f32,
    /// Na forma: a geometria (`a:prstGeom prst`: `rect`, `rightArrow`…) e o preenchimento (RGBA; 0 = nenhum).
    pub geometria: String,
    pub preenchimento: u32,
    /// No gráfico: o tipo (`barChart`, `lineChart`, `pieChart`…) e o nome de cada série.
    pub grafico: String,
    pub series: Vec<String>,
    /// No objeto OLE: o programa (`ProgID`, `Excel.Sheet.12`) e se vem incorporado (senão, vinculado).
    pub programa: String,
    pub incorporado: bool,
}

/// Lê o XML guardado de um objeto e, no gráfico, a parte dele (`word/charts/chart1.xml`).
pub fn info(xml: &str, partes: &[ParteWord]) -> InfoObjeto {
    let mut o = InfoObjeto::default();
    for tag in etiquetas(xml) {
        match nome(tag) {
            "wp:docPr" => o.nome = valor(tag, "name"),
            "wp:extent" => (o.largura, o.altura) = (emu(tag, "cx"), emu(tag, "cy")),
            "v:shape" | "v:rect" | "v:roundrect" | "v:oval" if o.largura == 0.0 => (o.largura, o.altura) = tamanho_vml(&valor(tag, "style")),
            "wps:wsp" if o.tipo == TipoDeObjeto::Outro => o.tipo = TipoDeObjeto::Forma,
            "wps:txbx" | "w:txbxContent" | "v:textbox" => o.tipo = TipoDeObjeto::CaixaDeTexto,
            "v:rect" | "v:roundrect" | "v:oval" | "v:line" if o.tipo == TipoDeObjeto::Outro => o.tipo = TipoDeObjeto::Forma,
            "a:prstGeom" => o.geometria = valor(tag, "prst"),
            "a:srgbClr" if o.preenchimento == 0 && o.tipo == TipoDeObjeto::Forma => o.preenchimento = u32::from_str_radix(&valor(tag, "val"), 16).map_or(0, |c| c << 8 | 0xFF),
            "c:chart" => o.tipo = TipoDeObjeto::Grafico,
            "dgm:relIds" => o.tipo = TipoDeObjeto::SmartArt,
            "o:OLEObject" => {
                o.tipo = TipoDeObjeto::Ole;
                o.programa = valor(tag, "ProgID");
                o.incorporado = valor(tag, "Type") != "Link";
            }
            _ => {}
        }
    }
    if o.tipo == TipoDeObjeto::Grafico {
        if let Some(p) = partes.iter().find(|p| p.caminho.contains("charts/chart")) {
            let x = String::from_utf8_lossy(&p.bytes);
            o.grafico = etiquetas(&x).map(nome).find(|n| n.starts_with("c:") && n.ends_with("Chart") && *n != "c:chart").unwrap_or("").trim_start_matches("c:").to_string();
            // O nome da série: o primeiro `c:v` dentro de cada `c:tx`.
            for ser in x.split("<c:ser>").skip(1) {
                let tx = ser.split("</c:tx>").next().unwrap_or("");
                if let Some(v) = tx.split("<c:v>").nth(1).and_then(|r| r.split("</c:v>").next()) {
                    o.series.push(desfazer(v));
                }
            }
        }
    }
    o
}

/// As etiquetas de abertura do XML (sem o `<`).
fn etiquetas(xml: &str) -> impl Iterator<Item = &str> {
    xml.split('<').skip(1).map(|t| t.split('>').next().unwrap_or("")).filter(|t| !t.starts_with('/') && !t.starts_with('!'))
}

fn nome(tag: &str) -> &str {
    tag.split([' ', '/']).next().unwrap_or("")
}

fn emu(tag: &str, nome: &str) -> f32 {
    valor(tag, nome).parse::<f32>().unwrap_or(0.0) / 12700.0
}

/// `width:100pt;height:50pt` em pt (só pt e in; o resto conta como pt).
fn tamanho_vml(estilo: &str) -> (f32, f32) {
    let medida = |k: &str| {
        let v = estilo.split(';').find_map(|d| d.trim().strip_prefix(k)?.trim().strip_prefix(':')).unwrap_or("").trim();
        let n: f32 = v.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(0.0);
        if v.ends_with("in") { n * 72.0 } else { n }
    };
    (medida("width"), medida("height"))
}

fn valor(tag: &str, nome: &str) -> String {
    tag.split(&format!(" {nome}=\"")).nth(1).map_or_else(String::new, |r| desfazer(r.split('"').next().unwrap_or("")))
}

fn desfazer(v: &str) -> String {
    v.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_forma_grafico_smartart_e_ole() {
        let f = info("<w:drawing><wp:inline><wp:extent cx=\"914400\" cy=\"457200\"/><wp:docPr id=\"6\" name=\"Seta\"/><a:graphic><a:graphicData><wps:wsp><wps:spPr><a:prstGeom prst=\"rightArrow\"/><a:solidFill><a:srgbClr val=\"FF0000\"/></a:solidFill></wps:spPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing>", &[]);
        assert_eq!((f.tipo, f.nome.as_str(), f.geometria.as_str(), f.preenchimento, f.largura, f.altura), (TipoDeObjeto::Forma, "Seta", "rightArrow", 0xFF0000FF, 72.0, 36.0));
        let parte = ParteWord { caminho: "word/charts/chart1.xml".into(), bytes: b"<c:chartSpace><c:chart><c:plotArea><c:barChart><c:ser><c:tx><c:v>Vendas</c:v></c:tx></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>".to_vec(), ..Default::default() };
        let g = info("<w:drawing><a:graphicData><c:chart r:id=\"rIdGraf\"/></a:graphicData></w:drawing>", &[parte]);
        assert_eq!((g.tipo, g.grafico.as_str(), g.series.as_slice()), (TipoDeObjeto::Grafico, "barChart", ["Vendas".to_string()].as_slice()));
        assert_eq!(info("<dgm:relIds r:dm=\"a\"/>", &[]).tipo, TipoDeObjeto::SmartArt);
        let o = info("<w:object><v:shape style=\"width:100pt;height:50pt\"><v:imagedata r:id=\"x\"/></v:shape><o:OLEObject Type=\"Embed\" ProgID=\"Excel.Sheet.12\"/></w:object>", &[]);
        assert_eq!((o.tipo, o.programa.as_str(), o.incorporado, o.largura, o.altura), (TipoDeObjeto::Ole, "Excel.Sheet.12", true, 100.0, 50.0));
    }
}
