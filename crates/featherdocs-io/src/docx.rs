//! `.docx` (Word, Google Docs, LibreOffice) para a árvore de blocos. O `.docx` é
//! um ZIP de XMLs; aqui só se lê o que vira conteúdo: `word/document.xml` (em
//! fluxo, sem montar a árvore DOM), `styles.xml` (o que é título, os estilos com
//! nome), `numbering.xml` (as listas do Word e a numeração dos títulos), as
//! relações (links, imagens, cabeçalho e rodapé), as imagens de `word/media/` e
//! `docProps/core.xml` (título e autor).
//!
//! Lê: títulos, parágrafos com a formatação de caractere (negrito, itálico,
//! sublinhado, tachado, fonte, tamanho, cor, realce, sobrescrito/subscrito) e de
//! parágrafo (alinhamento, espaços, entrelinha, recuos, fundo, bordas), listas com
//! nível e tipo de marcador, tabelas, links, imagens (embutidas viram `data:`),
//! fórmulas (o texto linear), sumário (campo `TOC`), legendas (campo `SEQ`), régua,
//! quebra de página, estilos com nome, papel e margens, cabeçalho e rodapé (com os
//! campos de página), cor e borda da página, alinhamento vertical e números de
//! linha. O que o modelo ainda não entende (forma, gráfico, SmartArt, objeto OLE, controle de
//! conteúdo) vai para a sacola (`docx/sacola.rs`) e volta igual. Não lê o `.doc` antigo (binário).
//! Gravar: `docx_gravar.rs`.

use std::collections::HashMap;
use std::io::Cursor;

use featherdocs_core::modelo::{classe_da_familia, AlturaDeLinha, BordasDeTabela, FioDeBorda, MesclagemTabela, PropsCelula, PropsTabela, Atrib, Celula, ConfigPagina, Conteudo, Documento, Estilo, Estilos, Faixas, Familia, Fmt, InicioDeSecao, Lista, ListaDoWord, Marca, NivelDeLista, Numeracao, NumeracaoDeLinhas, Revisao, TipoDeRevisao, Trecho, Vertical, Alinhamento, MaisLetra, MaisParagrafo, Tabulacao, ALINHAMENTO_DEFINIDO, CODIGO, DISTRIBUIDO, ITALICO, NEGRITO, QUEBRA_DE_LINHA, MATEMATICA, SERIFA, SUBLINHADO, TACHADO};
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

pub use crate::docx_gravar::gravar;
use crate::docx_gravar::{CAMPOS, MARCADORES_PADRAO, ROTULOS_LEGENDA};

type Zip<'a> = zip::ZipArchive<Cursor<&'a [u8]>>;

/// Lê os bytes de um `.docx`.
pub fn ler(bytes: &[u8]) -> Result<Documento, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Isto não é um .docx válido (o .doc antigo não é suportado): {e}"))?;
    let corpo = parte(&mut zip, "word/document.xml").ok_or("O .docx está incompleto: falta word/document.xml.")?;
    let mut numeracao = parte(&mut zip, "word/numbering.xml").map(|x| numeracao(&x)).unwrap_or_default();
    let tema = parte(&mut zip, "word/theme/theme1.xml").map(|x| String::from_utf8_lossy(&x).into_owned()).unwrap_or_default();
    let estilos = estilos_do_word(parte(&mut zip, "word/styles.xml").as_deref(), tema, &mut numeracao);
    let relacoes = parte(&mut zip, "word/_rels/document.xml.rels").map(|x| relacoes(&x)).unwrap_or_default();
    let (comentarios, respostas) = match parte(&mut zip, "word/comments.xml") {
        Some(c) => crate::docx_comentarios::ler(&c, parte(&mut zip, "word/commentsExtended.xml").as_deref()),
        None => (Vec::new(), Vec::new()),
    };
    let mut l = Leitor::novo(estilos, numeracao, relacoes);
    l.respostas = respostas;
    l.sacola = true;
    let mut leitor = Reader::from_reader(corpo.as_slice());
    loop {
        match leitor.read_event() {
            Ok(Event::Eof) => break,
            Ok(e) => l.evento(e),
            Err(e) => return Err(format!("O XML do .docx está malformado (posição {}): {e}", leitor.error_position())),
        }
    }
    l.estilos_do_documento();
    let mut notas = HashMap::new();
    for (k, nome) in ["word/footnotes.xml", "word/endnotes.xml"].into_iter().enumerate() {
        if let Some(x) = parte(&mut zip, nome) {
            notas.extend(ler_notas(&x, l.estilos.clone(), k == 1));
        }
    }
    ligar_notas(&mut l.doc, &notas);
    l.doc.comentarios = comentarios;
    if let Some(x) = parte(&mut zip, "word/settings.xml") {
        let s = String::from_utf8_lossy(&x);
        l.doc.extras.controlar_alteracoes = s.find("<w:trackRevisions").is_some_and(|i| {
            let tag = s[i..].split('>').next().unwrap_or("");
            !tag.contains("w:val=\"0\"") && !tag.contains("w:val=\"false\"")
        });
        l.doc.extras.protegido = s.find("<w:documentProtection").is_some_and(|i| {
            let tag = s[i..].split('>').next().unwrap_or("");
            tag.contains("readOnly") && (tag.contains("w:enforcement=\"1\"") || tag.contains("w:enforcement=\"true\"") || tag.contains("w:enforcement=\"on\""))
        });
        let m = &mut l.doc.extras.documento;
        m.hifenizar = s.find("<w:autoHyphenation").is_some_and(|i| {
            let tag = s[i..].split('>').next().unwrap_or("");
            !tag.contains("w:val=\"0\"") && !tag.contains("w:val=\"false\"")
        });
        // 708 twips (1,25 cm) é o padrão do Word: fica 0.
        m.tabulacao = valor_da_tag(&s, "<w:defaultTabStop").and_then(|v| v.parse::<f32>().ok()).filter(|v| (*v - 708.0).abs() > 0.5).map_or(0.0, |v| v / 20.0);
    }
    let faixas = [l.cabecalho.take(), l.rodape.take(), l.cabecalho_primeira.take(), l.rodape_primeira.take()]
        .map(|rel| rel.and_then(|id| l.relacoes.get(&id).cloned()).and_then(|alvo| faixa_da_parte(&mut zip, &caminho_em_word(&alvo))));
    let mut doc = l.doc;
    doc.pagina = papel_conhecido(doc.pagina);
    let [cabecalho, rodape, cabecalho_primeira, rodape_primeira] = faixas;
    // A marca d'água mora no cabeçalho (o de sempre, ou o da primeira página).
    if let Some((texto, cor)) = cabecalho.as_ref().and_then(|f| f.marca.clone()).or_else(|| cabecalho_primeira.as_ref().and_then(|f| f.marca.clone())) {
        doc.extras.marca = texto;
        doc.extras.marca_cor = cor;
    }
    // O timbre do cabeçalho de sempre volta como figura de toda página (o da primeira página repete o mesmo).
    if let Some(f) = cabecalho.as_ref() {
        doc.extras.decoracoes.splice(0..0, f.timbre.iter().cloned());
    }
    doc.extras.cabecalho = cabecalho.map(|f| f.faixa).unwrap_or_default();
    doc.extras.rodape = rodape.map(|f| f.faixa).unwrap_or_default();
    doc.extras.cabecalho_primeira = cabecalho_primeira.map(|f| f.faixa).unwrap_or_default();
    doc.extras.rodape_primeira = rodape_primeira.map(|f| f.faixa).unwrap_or_default();
    // A distância igual à automática (a que o FeatherDocs grava) volta a ser automática.
    let x = &mut doc.extras;
    let auto = featherdocs_core::modelo::distancia_automatica;
    if (x.documento.distancia_cabecalho - auto(&doc.pagina, &x.cabecalho, true)).abs() < 0.5 {
        x.documento.distancia_cabecalho = 0.0;
    }
    if (x.documento.distancia_rodape - auto(&doc.pagina, &x.rodape, false)).abs() < 0.5 {
        x.documento.distancia_rodape = 0.0;
    }
    unir_codigo(&mut doc);
    embutir_imagens(&mut doc, &mut zip);
    sacola::embutir(&mut doc, &mut zip);
    if let Some(x) = parte(&mut zip, "docProps/core.xml") {
        propriedades(&x, &mut doc);
    }
    if let Some(x) = parte(&mut zip, "docProps/app.xml") {
        propriedades(&x, &mut doc);
    }
    if let Some(x) = parte(&mut zip, "docProps/custom.xml") {
        doc.extras.documento.personalizadas = personalizadas(&x);
    }
    if let Some(x) = parte(&mut zip, "word/styles.xml") {
        // O idioma padrão: o `w:lang` do `w:docDefaults` (pt-BR fica vazio, o nosso padrão).
        let s = String::from_utf8_lossy(&x);
        let padrao = s.find("<w:docDefaults").map(|i| &s[i..s[i..].find("</w:docDefaults>").map_or(s.len(), |j| i + j)]).unwrap_or("");
        doc.extras.documento.idioma = valor_da_tag(padrao, "<w:lang").filter(|v| v != "pt-BR").unwrap_or_default();
    }
    let itens: Vec<String> = zip.file_names().filter(|n| n.starts_with("customXml/item") && !n.contains("Props") && n.ends_with(".xml")).map(str::to_string).collect();
    for nome in itens {
        if let Some(b) = parte(&mut zip, &nome).and_then(|x| crate::docx_fontes::ler(&x)) {
            doc.bibliografia = b;
        }
    }
    doc.acertar_ids();
    Ok(doc)
}

/// O texto de cada nota de `footnotes.xml` ou `endnotes.xml`, pela chave `(fim, w:id)`.
fn ler_notas(xml: &[u8], estilos: EstilosDocx, fim: bool) -> HashMap<(bool, String), Vec<Trecho>> {
    let mut l = Leitor::novo(estilos, Numeracoes::default(), HashMap::new());
    let mut leitor = Reader::from_reader(xml);
    while let Ok(e) = leitor.read_event() {
        if matches!(e, Event::Eof) {
            break;
        }
        l.evento(e);
    }
    l.notas.into_iter().map(|(id, t)| ((fim, id), t)).collect()
}

/// Põe o texto de cada nota na chamada dela (que guardou o `w:id` no rótulo durante a leitura).
fn ligar_notas(doc: &mut Documento, notas: &HashMap<(bool, String), Vec<Trecho>>) {
    for b in &mut doc.blocos {
        for t in b.conteudo.textos_mut() {
            for tr in t.iter_mut() {
                if let Some(Marca::Nota { fim, texto }) = tr.marca.as_deref_mut() {
                    *texto = notas.get(&(*fim, tr.rotulo.clone())).cloned().unwrap_or_default();
                    tr.rotulo.clear();
                }
            }
        }
    }
}

/// O objeto que um campo do Word vira: entrada de índice (`XE`), campo de dados ou citação.
fn objeto_do_campo(instr: &str) -> Option<Marca> {
    let palavras: Vec<&str> = instr.split_whitespace().collect();
    let entre_aspas = || instr.split('"').nth(1).unwrap_or("").to_string();
    Some(match palavras.first()?.to_ascii_uppercase().as_str() {
        "XE" => {
            let e = entre_aspas();
            let (termo, sub) = e.split_once(':').unwrap_or((&e, ""));
            Marca::Indice { termo: termo.to_string(), sub: sub.to_string() }
        }
        "AUTHOR" => Marca::Campo { campo: "autor".into() },
        "TITLE" => Marca::Campo { campo: "titulo".into() },
        "DATE" | "CREATEDATE" | "SAVEDATE" => Marca::Campo { campo: "data".into() },
        "PAGE" => Marca::Campo { campo: "pagina".into() },
        "NUMPAGES" => Marca::Campo { campo: "paginas".into() },
        "FILENAME" => Marca::Campo { campo: "arquivo".into() },
        "CITATION" => {
            let mut chaves = Vec::new();
            let mut pagina = String::new();
            let mut k = 1;
            while k < palavras.len() {
                match palavras[k] {
                    "\\m" => {}
                    "\\p" => {
                        pagina = palavras.get(k + 1).unwrap_or(&"").to_string();
                        k += 1;
                    }
                    p if p.starts_with('\\') => k += 1,
                    p => chaves.push(p.to_string()),
                }
                k += 1;
            }
            Marca::Citacao { chaves, pagina }
        }
        _ => return None,
    })
}

/// O Word guarda o papel em vigésimos de ponto: A4 e Carta voltam às medidas exatas.
fn papel_conhecido(mut p: ConfigPagina) -> ConfigPagina {
    for conhecido in [ConfigPagina::a4(), ConfigPagina::carta()] {
        for (l, a) in [(conhecido.largura, conhecido.altura), (conhecido.altura, conhecido.largura)] {
            if (p.largura - l).abs() < 0.1 && (p.altura - a).abs() < 0.1 {
                p.largura = l;
                p.altura = a;
            }
        }
    }
    p
}

fn parte(zip: &mut Zip, nome: &str) -> Option<Vec<u8>> {
    crate::ler_limitado(zip.by_name(nome).ok()?).ok()
}

/// O `w:numFmt` das notas no código do modelo (`Extras::formato_notas`).
fn formato_da_nota(v: Option<&str>, padrao: u8) -> u8 {
    match v {
        Some("decimal") => 0,
        Some("lowerLetter") => 1,
        Some("lowerRoman") => 2,
        Some("chicago") => 3,
        _ => padrao,
    }
}

/// O alvo de uma relação de `word/document.xml` como caminho dentro do ZIP.
fn caminho_em_word(alvo: &str) -> String {
    match alvo.strip_prefix('/') {
        Some(absoluto) => absoluto.to_string(),
        None => format!("word/{}", alvo.trim_start_matches("./")),
    }
}

/// A etiqueta de um elemento de dentro do `m:oMath`, com o `m:val` (o resto dos atributos não importa).
fn etiqueta_math(e: &BytesStart, vazio: bool) -> String {
    let nome = nome_de(e);
    let val = atributo(e, "m:val").map_or(String::new(), |v| format!(" m:val=\"{}\"", esc_math(&v)));
    format!("<{nome}{val}{}>", if vazio { "/" } else { "" })
}

fn esc_math(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub(crate) fn atributo(e: &BytesStart, nome: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == nome).and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok()).map(|v| v.into_owned())
}

fn numero(e: &BytesStart, nome: &str) -> Option<f32> {
    atributo(e, nome).and_then(|v| v.parse::<f32>().ok())
}

fn nome_de(e: &BytesStart) -> String {
    e.name().as_ref().to_string()
}

/// `w:b`, `w:i`… estão ligados salvo `w:val="0"`/`false`/`none`.
fn ligado(e: &BytesStart) -> bool {
    !matches!(atributo(e, "w:val").as_deref(), Some("0" | "false" | "none" | "off"))
}

/// `RRGGBB` para `0xRRGGBBFF`; `auto` e lixo = 0 (automática).
fn cor(v: &str) -> u32 {
    if v.len() != 6 {
        return 0;
    }
    u32::from_str_radix(v, 16).map(|c| (c << 8) | 0xFF).unwrap_or(0)
}

/// As cores com nome do `w:highlight`.
fn cor_do_realce(nome: &str) -> u32 {
    let rgb = match nome {
        "yellow" => "FFFF00",
        "green" => "00FF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "blue" => "0000FF",
        "red" => "FF0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "C0C0C0",
        "black" => "000000",
        "white" => "FFFFFF",
        _ => return 0,
    };
    cor(rgb)
}

/// `&amp;`, `&#233;`… como texto.
pub(crate) fn referencia(r: &BytesRef) -> String {
    match r.resolve_char_ref() {
        Ok(Some(c)) => c.to_string(),
        _ => match r.as_ref() {
            "amp" => "&".into(),
            "lt" => "<".into(),
            "gt" => ">".into(),
            "quot" => "\"".into(),
            "apos" => "'".into(),
            _ => String::new(),
        },
    }
}

fn twips_para_pt(e: &BytesStart, nome: &str) -> Option<f32> {
    numero(e, nome).map(|v| v / 20.0)
}

// ---------------------------------------------------------------------------
// Propriedades compartilhadas por estilos e corpo
// ---------------------------------------------------------------------------

/// Uma propriedade de caractere (`w:rPr`). `false` se não é uma delas. Do nome da fonte (o mapa o
/// lê) sai a classe, os bits `SERIFA`/`CODIGO` com que o motor mede; `tema` = as fontes do tema
/// (`+majorHAnsi`, `+minorHAnsi`).
fn prop_caractere(nome: &str, e: &BytesStart, estilo: &mut u8, fmt: &mut Fmt, tema: &(String, String)) -> bool {
    let antes = fmt.familia;
    if nome == "w:rFonts" {
        fmt.familia = Familia::default();
    }
    let mut c = (*estilo, *fmt);
    let achou = mapa::ler(mapa::CARACTERE, nome, e, &mut c);
    (*estilo, *fmt) = c;
    if nome == "w:rFonts" {
        if fmt.familia.vazia() {
            fmt.familia = antes;
            return achou;
        }
        let real = match fmt.familia.do_tema() {
            Some(t) if t.starts_with("major") => tema.0.clone(),
            Some(_) => tema.1.clone(),
            None => fmt.familia.nome().to_string(),
        };
        *estilo = (*estilo & !(SERIFA | CODIGO)) | classe_da_familia(&real);
        // A fonte que o gravador põe pela classe (Times New Roman, Courier New) é só a classe: assim o
        // trecho do FeatherDocs volta igual.
        if [crate::docx_gravar::FONTE_SERIFA, crate::docx_gravar::FONTE_CODIGO].iter().any(|f| f.eq_ignore_ascii_case(fmt.familia.nome())) {
            fmt.familia = Familia::default();
        }
    }
    achou
}

/// Uma propriedade de parágrafo (`w:pPr`): alinhamento, espaços, recuos, fundo.
fn prop_paragrafo(nome: &str, e: &BytesStart, a: &mut Atrib) {
    mapa::ler(mapa::PARAGRAFO, nome, e, a);
}

/// Um lado de `w:pBdr`.
fn borda_do_paragrafo(nome: &str, e: &BytesStart, a: &mut Atrib) {
    let bit = match nome {
        "w:left" | "w:start" => 1,
        "w:right" | "w:end" => 2,
        "w:top" => 4,
        "w:bottom" => 8,
        _ => return,
    };
    if matches!(atributo(e, "w:val").as_deref(), Some("nil" | "none")) {
        return;
    }
    a.borda |= bit;
    if let Some(c) = atributo(e, "w:color").map(|v| cor(&v)).filter(|c| *c != 0) {
        a.cor_borda = c;
    }
    // O traço simples de 0,75 pt (o que o gravador põe sem nada dito) fica como zero.
    a.mais.borda_tipo = atributo(e, "w:val").and_then(|v| mapa::TRACOS_DE_BORDA.iter().position(|t| *t == v)).unwrap_or(0) as u8;
    a.mais.borda_espessura = numero(e, "w:sz").map_or(0.0, |sz| sz / 8.0);
    if a.mais.borda_espessura == ESPESSURA_DA_BORDA {
        a.mais.borda_espessura = 0.0;
    }
}

/// A espessura da borda do parágrafo que o gravador põe quando o modelo não diz outra (`w:sz="6"`).
pub(crate) const ESPESSURA_DA_BORDA: f32 = 0.75;

/// Uma parada de `w:tabs`.
fn tabulacao(e: &BytesStart, a: &mut Atrib) {
    let codigo = |lista: &[&str], nome: &str| atributo(e, nome).and_then(|v| lista.iter().position(|t| *t == v)).unwrap_or(0) as u8;
    let alinhamento = match atributo(e, "w:val").as_deref() {
        Some("start") => 0,
        Some("end") => 2,
        _ => codigo(&mapa::ALINHAMENTOS_DE_TABULACAO, "w:val"),
    };
    let posicao = numero(e, "w:pos").unwrap_or(0.0) / 20.0;
    a.mais.acrescentar_tabulacao(Tabulacao { posicao, alinhamento, preenchimento: codigo(&mapa::PREENCHIMENTOS_DE_TABULACAO, "w:leader") });
}

// ---------------------------------------------------------------------------
// Estilos
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct EstiloDocx {
    /// O nome em minúsculas (para reconhecer "heading 1", "quote"…).
    nome: String,
    nome_original: String,
    /// `paragraph`, `character`, `table`, `numbering`.
    tipo: String,
    /// `w:default="1"` (o Normal, o "Default Paragraph Font").
    padrao: bool,
    nivel: Option<u8>,
    base: Option<String>,
    proximo: Option<String>,
    numerado: bool,
    /// O `w:numPr` do estilo: o `numId` (a lista que o estilo põe, ou a do estilo de lista) e o nível.
    num_id: Option<String>,
    ilvl: u8,
    bits: u8,
    fmt: Fmt,
    atrib: Atrib,
    /// O que o estilo desliga à vista (`Estilo::desligados`).
    desligados: u32,
}

impl EstiloDocx {
    /// A mesma letra e o mesmo parágrafo (a numeração e o próximo não contam).
    fn mesma_aparencia(&self, o: &EstiloDocx) -> bool {
        (self.tipo.as_str(), self.nivel, self.bits, self.fmt, self.atrib, self.desligados) == (o.tipo.as_str(), o.nivel, o.bits, o.fmt, o.atrib, o.desligados)
    }
}

/// Os bits de liga/desliga que uma propriedade do estilo desliga à vista (`<w:b w:val="0"/>`): lida
/// também sobre tudo ligado, o bit que apaga é o desligado.
fn desligados_da_propriedade(nome: &str, e: &BytesStart, em_ppr: bool, tema: &(String, String)) -> u32 {
    if em_ppr {
        let mut a = Atrib { mais: MaisParagrafo { fluxo: 0xFF, ..MaisParagrafo::default() }, ..Atrib::default() };
        prop_paragrafo(nome, e, &mut a);
        // O `w:jc` só desfaz o do pai à vista quando é à esquerda (os outros já valem por si); o
        // distribuído só se desliga num justificado comum.
        let (alinhado, distribuido) = match a.alinhamento {
            Alinhamento::Esquerda if nome == "w:jc" => (ALINHAMENTO_DEFINIDO, 0),
            Alinhamento::Justificado => (0, 0),
            _ => (0, DISTRIBUIDO),
        };
        return u32::from(!a.mais.fluxo & !distribuido) << 16 | alinhado;
    }
    let (mut bits, mut f) = (0xFF, Fmt { mais: MaisLetra { efeitos: 0xFF, ..MaisLetra::default() }, ..Fmt::default() });
    prop_caractere(nome, e, &mut bits, &mut f, tema);
    // A classe da fonte (`SERIFA`, `CODIGO`) não se desliga: a fonte do filho a troca.
    u32::from(!bits & (NEGRITO | ITALICO | SUBLINHADO | TACHADO)) | u32::from(!f.mais.efeitos) << 8
}

/// O `styles.xml` lido: `styleId` → estilo, a ordem do arquivo e o `w:docDefaults`.
#[derive(Default, Clone)]
struct EstilosLidos {
    mapa: HashMap<String, EstiloDocx>,
    ordem: Vec<String>,
    padrao: EstiloDocx,
}

/// Os estilos do `.docx`, como o leitor do corpo usa: os lidos (pelo `styleId` do arquivo), o id de
/// cada um no modelo, os estilos do modelo (com a herança, para resolver) e o tema.
#[derive(Default, Clone)]
struct EstilosDocx {
    mapa: HashMap<String, EstiloDocx>,
    ids: HashMap<String, u32>,
    modelo: Estilos,
    /// As fontes do tema (`a:majorFont`, `a:minorFont`).
    fontes_do_tema: (String, String),
    /// O que o parágrafo sem estilo (o Normal) traz de alinhamento, espaços e recuos.
    atrib_normal: Atrib,
}

impl EstilosDocx {
    fn get(&self, id: &str) -> Option<&EstiloDocx> {
        self.mapa.get(id)
    }
}

/// `styleId` → estilo (nível de título pelo nome, por `outlineLvl` ou pelo estilo de base).
fn estilos(xml: &[u8], tema: &(String, String)) -> EstilosLidos {
    let mut saida = EstilosLidos::default();
    let mut leitor = Reader::from_reader(xml);
    let mut atual: Option<(String, EstiloDocx)> = None;
    let mut em_ppr = false;
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let nome = nome_de(e);
                if nome == "w:style" {
                    let tipo = atributo(e, "w:type").unwrap_or_else(|| "paragraph".into());
                    let padrao = matches!(atributo(e, "w:default").as_deref(), Some("1" | "true" | "on"));
                    atual = atributo(e, "w:styleId").map(|id| (id, EstiloDocx { tipo, padrao, ..EstiloDocx::default() }));
                } else if nome == "w:docDefaults" {
                    atual = Some((String::new(), EstiloDocx::default()));
                } else if nome == "w:pPr" {
                    em_ppr = matches!(ev, Event::Start(_));
                } else if let Some((_, s)) = &mut atual {
                    propriedade_de_estilo(&nome, e, s, em_ppr, tema);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "w:style" => {
                    if let Some((id, s)) = atual.take() {
                        saida.ordem.push(id.clone());
                        saida.mapa.insert(id, s);
                    }
                }
                "w:docDefaults" => {
                    if let Some((_, s)) = atual.take() {
                        saida.padrao = s;
                    }
                }
                "w:pPr" => em_ppr = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    resolver_niveis(&mut saida.mapa);
    saida
}

fn propriedade_de_estilo(nome: &str, e: &BytesStart, s: &mut EstiloDocx, em_ppr: bool, tema: &(String, String)) {
    match nome {
        "w:name" => {
            s.nome_original = atributo(e, "w:val").unwrap_or_default();
            s.nome = s.nome_original.to_lowercase();
            s.nivel = s.nivel.or(nivel_do_nome(&s.nome));
        }
        "w:basedOn" => s.base = atributo(e, "w:val"),
        "w:tab" if em_ppr => tabulacao(e, &mut s.atrib),
        "w:next" => s.proximo = atributo(e, "w:val"),
        "w:outlineLvl" => {
            if let Some(v) = atributo(e, "w:val").and_then(|v| v.parse::<u8>().ok()).filter(|v| *v < 9) {
                s.nivel.get_or_insert((v + 1).min(6));
            }
        }
        "w:numId" => {
            s.num_id = atributo(e, "w:val").filter(|v| v != "0");
            s.numerado = s.num_id.is_some();
        }
        "w:ilvl" => s.ilvl = atributo(e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(0).min(8),
        _ if em_ppr => {
            prop_paragrafo(nome, e, &mut s.atrib);
            s.desligados |= desligados_da_propriedade(nome, e, true, tema);
        }
        _ => {
            prop_caractere(nome, e, &mut s.bits, &mut s.fmt, tema);
            s.desligados |= desligados_da_propriedade(nome, e, false, tema);
        }
    }
}

/// O `styleId` que o gravador usa para os estilos que ele mesmo conhece (o do arquivo é traduzido:
/// "Ttulo1" do Word em português é o "Heading1"); os outros ficam como vieram.
fn codigo_canonico(id: &str, nome: &str) -> String {
    if let Some(n) = nome.strip_prefix("heading ").and_then(|n| n.trim().parse::<u8>().ok()).filter(|n| (1..=9).contains(n)) {
        return format!("Heading{n}");
    }
    let conhecidos = [("normal", "Normal"), ("title", "Title"), ("subtitle", "Subtitle"), ("quote", "Quote"), ("caption", "Caption"), ("hyperlink", "Hyperlink"), ("list paragraph", "ListParagraph"), ("toc heading", "TOCHeading")];
    conhecidos.iter().find(|(n, _)| *n == nome).map_or_else(|| id.to_string(), |(_, c)| (*c).to_string())
}

/// Os estilos do `.docx` no modelo. Entram os de parágrafo e de caractere, menos os que são iguais
/// aos que o gravador do FeatherDocs põe por conta própria (assim o documento do FeatherDocs volta
/// sem estilos a mais); o `FDEstilo<n>` volta com o id `n`, os outros ganham ids depois deles.
fn estilos_do_word(xml: Option<&[u8]>, tema: String, numeracao: &mut Numeracoes) -> EstilosDocx {
    let modelo = Estilos { tema, ..Estilos::default() };
    let fontes_do_tema = modelo.fontes_do_tema();
    let lidos = xml.map(|x| estilos(x, &fontes_do_tema)).unwrap_or_default();
    numeracao.estilos_de_lista = lidos.mapa.iter().filter(|(_, s)| s.tipo == "numbering").filter_map(|(id, s)| Some((id.clone(), s.num_id.clone()?))).collect();
    let embutidos = estilos(crate::docx_gravar::estilos_embutidos().as_bytes(), &(String::new(), String::new()));
    let do_fd = |id: &str| id.strip_prefix("FDEstilo").and_then(|n| n.parse::<u32>().ok());
    let entra = |id: &String| {
        let s = &lidos.mapa[id];
        if !matches!(s.tipo.as_str(), "paragraph" | "character") || s.tipo == "character" && s.padrao {
            return false;
        }
        let codigo = codigo_canonico(id, &s.nome);
        !embutidos.mapa.get(&codigo).is_some_and(|b| b.mesma_aparencia(s))
    };
    let escolhidos: Vec<&String> = lidos.ordem.iter().filter(|id| lidos.mapa.contains_key(*id) && entra(id)).collect();
    let mut proximo = escolhidos.iter().filter_map(|id| do_fd(id)).max().unwrap_or(0);
    let mut ids = HashMap::new();
    for id in &escolhidos {
        let n = do_fd(id).unwrap_or_else(|| {
            proximo += 1;
            proximo
        });
        ids.insert((*id).clone(), n);
    }
    let mut lista: Vec<Estilo> = escolhidos
        .iter()
        .map(|id| {
            let s = &lidos.mapa[*id];
            let mut e = Estilo {
                id: ids[*id],
                nome: s.nome_original.clone(),
                base: s.nivel.unwrap_or(0),
                codigo: if do_fd(id).is_some() { String::new() } else { codigo_canonico(id, &s.nome) },
                caractere: s.tipo == "character",
                herda: s.base.as_ref().and_then(|b| ids.get(b)).copied().unwrap_or(0),
                proximo: s.proximo.as_ref().and_then(|b| ids.get(b)).copied().unwrap_or(0),
                ..Estilo::default()
            };
            e.por_caractere((s.bits, s.fmt));
            e.por_paragrafo(&s.atrib);
            e.desligados = s.desligados;
            // O estilo que numera (List Number): o parágrafo com ele vira item da lista.
            if let Some(l) = s.num_id.as_deref().filter(|_| s.nivel.is_none() && s.tipo == "paragraph").and_then(|n| numeracao.lista_do_modelo(n, true)).filter(|l| *l != 0) {
                (e.mais_paragrafo.lista, e.mais_paragrafo.nivel_lista) = (l, s.ilvl);
            }
            e
        })
        .collect();
    lista.sort_by_key(|e| e.id);
    let mut modelo = Estilos { lista, ..modelo };
    if !lidos.padrao.mesma_aparencia(&embutidos.padrao) {
        modelo.padrao.por_caractere((lidos.padrao.bits, lidos.padrao.fmt));
        modelo.padrao.por_paragrafo(&lidos.padrao.atrib);
        modelo.padrao.desligados = lidos.padrao.desligados;
    }
    let atrib_normal = modelo.resolvido_do_bloco(&featherdocs_core::modelo::Bloco::novo(0, Conteudo::Paragrafo { texto: Vec::new() })).map(|e| e.paragrafo_do_estilo()).unwrap_or_default();
    EstilosDocx { mapa: lidos.mapa, ids, modelo, fontes_do_tema, atrib_normal }
}

/// O estilo sem nível próprio herda o do estilo de base (um "Meu título" baseado em "heading 2" é título 2).
fn resolver_niveis(estilos: &mut HashMap<String, EstiloDocx>) {
    let ids: Vec<String> = estilos.keys().cloned().collect();
    for id in ids {
        let mut atual = estilos.get(&id).and_then(|s| s.base.clone());
        let mut passos = 0;
        while estilos.get(&id).is_some_and(|s| s.nivel.is_none()) && passos < 10 {
            let Some(b) = atual else { break };
            let base = estilos.get(&b).map(|s| (s.nivel, s.base.clone()));
            let Some((nivel, proxima)) = base else { break };
            if let (Some(n), Some(s)) = (nivel, estilos.get_mut(&id)) {
                s.nivel = Some(n);
            }
            atual = proxima;
            passos += 1;
        }
    }
}

fn nivel_do_nome(nome: &str) -> Option<u8> {
    if nome == "title" {
        return Some(1);
    }
    if nome == "subtitle" {
        return Some(2);
    }
    let n = nome.strip_prefix("heading ").or_else(|| nome.strip_prefix("título "))?;
    n.trim().parse::<u8>().ok().filter(|n| (1..=9).contains(n)).map(|n| n.min(6))
}

// ---------------------------------------------------------------------------
// Numeração
// ---------------------------------------------------------------------------

/// O que uma lista é, num nível: numerada ou não, e o estilo do marcador.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TipoDeLista {
    numerada: bool,
    lista: Lista,
    marcado: bool,
}

#[derive(Default)]
struct Numeracoes {
    /// `numId` → `(abstractNumId, startOverride por nível)`.
    nums: HashMap<String, (String, HashMap<u8, u32>)>,
    /// `abstractNumId` → os níveis lidos e o `w:numStyleLink`.
    abstratas: HashMap<String, (HashMap<u8, NivelLido>, Option<String>)>,
    /// O `styleId` de cada estilo de lista → o `numId` dele (para seguir o `w:numStyleLink`).
    estilos_de_lista: HashMap<String, String>,
    /// As listas do Word que entram no modelo (`Estilos::listas`) e o índice + 1 de cada `abstractNumId`
    /// nelas (0 = igual a uma lista que o FeatherDocs grava: fica lista do FeatherDocs).
    modelo: Vec<ListaDoWord>,
    indices: HashMap<String, u16>,
    /// `(numId, nível)` que já tiveram item: o `startOverride` vale só no primeiro.
    vistos: std::collections::HashSet<(String, u8)>,
    /// A numeração dos títulos (um nível ligado a um estilo por `w:pStyle`): "Capítulo %1" ou "%1".
    titulos: Option<Numeracao>,
}

impl Numeracoes {
    /// Os níveis do `abstractNumId` (o `w:numStyleLink` seguido até a lista do estilo), do 0 ao último.
    fn niveis(&self, abs: &str) -> Vec<NivelDeLista> {
        let mut abs = abs;
        for _ in 0..4 {
            let Some((niveis, link)) = self.abstratas.get(abs) else { return Vec::new() };
            if niveis.is_empty() {
                if let Some(a) = link.as_ref().and_then(|l| self.estilos_de_lista.get(l)).and_then(|n| self.nums.get(n)) {
                    abs = &a.0;
                    continue;
                }
            }
            let ultimo = niveis.keys().max().map_or(0, |n| usize::from(*n) + 1);
            return (0..ultimo).map(|i| niveis.get(&(i as u8)).map(|n| n.def.clone()).unwrap_or_default()).collect();
        }
        Vec::new()
    }

    /// A lista do modelo (índice + 1; 0 = a do FeatherDocs) do `numId`; `None` se ele não existe.
    /// `forcar` = sempre como lista do Word (a de um estilo de parágrafo, que o FeatherDocs não grava).
    fn lista_do_modelo(&mut self, num: &str, forcar: bool) -> Option<u16> {
        let abs = self.nums.get(num)?.0.clone();
        if let Some(i) = self.indices.get(&abs).filter(|i| **i != 0 || !forcar) {
            return Some(*i);
        }
        let niveis = self.niveis(&abs);
        let nativa = !niveis.is_empty() && (0..crate::docx_gravar::TIPOS_NATIVOS).any(|t| niveis.iter().enumerate().all(|(n, x)| *x == crate::docx_gravar::nivel_nativo(t, n as u8)));
        let i = if niveis.is_empty() || nativa && !forcar || self.modelo.len() >= usize::from(u16::MAX) {
            0
        } else {
            // O `w:numStyleLink` (a lista sem níveis próprios) volta como estilo de lista.
            let estilo = self.abstratas.get(&abs).filter(|a| a.0.is_empty()).and_then(|a| a.1.clone()).unwrap_or_default();
            self.modelo.push(ListaDoWord { niveis, estilo });
            self.modelo.len() as u16
        };
        self.indices.insert(abs, i);
        Some(i)
    }

    /// O `startOverride` do nível, se este é o primeiro item do `numId` nele.
    fn reinicio(&mut self, num: &str, ilvl: u8) -> Option<u32> {
        let novo = self.vistos.insert((num.to_string(), ilvl));
        self.nums.get(num).and_then(|n| n.1.get(&ilvl)).copied().filter(|_| novo)
    }
}

/// O que a lista é no nível: numerada ou não, e o estilo do marcador. O marcador do nível 0 diz o
/// estilo da lista toda ("•" = o padrão, que varia por nível).
fn tipo_da_lista(niveis: &[NivelDeLista], ilvl: u8) -> TipoDeLista {
    let primeiro = niveis.first().map_or("", |n| n.texto.as_str());
    let Some(n) = niveis.get(usize::from(ilvl)) else { return TipoDeLista::default() };
    let texto = if n.formato == "bullet" && primeiro != MARCADORES_PADRAO[0] { primeiro } else { n.texto.as_str() };
    tipo_do_nivel(&n.formato, texto, if texto == primeiro { 0 } else { ilvl })
}

fn tipo_do_nivel(fmt: &str, texto: &str, ilvl: u8) -> TipoDeLista {
    let numerada = !matches!(fmt, "bullet" | "none" | "");
    let lista = match fmt {
        "lowerLetter" | "upperLetter" => Lista::Alfabetica,
        "lowerRoman" => Lista::RomanaMinuscula,
        "upperRoman" => Lista::Romana,
        _ if numerada => Lista::Padrao,
        _ if texto == MARCADORES_PADRAO[usize::from(ilvl) % 3] => Lista::Padrao,
        _ => match texto {
            "◦" | "o" => Lista::Circulo,
            "▪" | "■" | "§" => Lista::Quadrado,
            "➢" | "►" | "→" | "Ø" => Lista::Seta,
            "☐" | "☒" | "☑" => Lista::Tarefa,
            _ => Lista::Padrao,
        },
    };
    TipoDeLista { numerada, lista, marcado: texto == "☒" || texto == "☑" }
}

#[derive(Default)]
struct NivelLido {
    def: NivelDeLista,
    estilo: Option<String>,
}

fn nivel_lido<'a>(s: &'a mut Numeracoes, abs: &str, ilvl: u8) -> &'a mut NivelLido {
    s.abstratas.entry(abs.to_string()).or_default().0.entry(ilvl).or_default()
}

fn numeracao(xml: &[u8]) -> Numeracoes {
    let mut saida = Numeracoes::default();
    let mut leitor = Reader::from_reader(xml);
    let (mut abs_id, mut num_id, mut ilvl, mut sobre) = (String::new(), String::new(), 0u8, 0u8);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let nome = nome_de(e);
                match nome.as_str() {
                    "w:abstractNum" => {
                        abs_id = atributo(e, "w:abstractNumId").unwrap_or_default();
                        saida.abstratas.entry(abs_id.clone()).or_default();
                        num_id.clear();
                    }
                    "w:numStyleLink" if num_id.is_empty() => saida.abstratas.entry(abs_id.clone()).or_default().1 = atributo(e, "w:val"),
                    "w:lvl" if num_id.is_empty() => {
                        ilvl = atributo(e, "w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0).min(8);
                        let _ = nivel_lido(&mut saida, &abs_id, ilvl);
                    }
                    "w:start" if num_id.is_empty() => nivel_lido(&mut saida, &abs_id, ilvl).def.inicio = atributo(e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(0),
                    "w:numFmt" if num_id.is_empty() => nivel_lido(&mut saida, &abs_id, ilvl).def.formato = atributo(e, "w:val").unwrap_or_default(),
                    "w:lvlText" if num_id.is_empty() => nivel_lido(&mut saida, &abs_id, ilvl).def.texto = atributo(e, "w:val").unwrap_or_default(),
                    "w:pStyle" if num_id.is_empty() => nivel_lido(&mut saida, &abs_id, ilvl).estilo = atributo(e, "w:val"),
                    "w:ind" if num_id.is_empty() => {
                        let n = nivel_lido(&mut saida, &abs_id, ilvl);
                        n.def.recuo = numero(e, "w:left").or_else(|| numero(e, "w:start")).unwrap_or(0.0) / 20.0;
                        n.def.pendurado = numero(e, "w:hanging").unwrap_or(0.0) / 20.0;
                    }
                    "w:rFonts" if num_id.is_empty() => nivel_lido(&mut saida, &abs_id, ilvl).def.fonte = atributo(e, "w:ascii").or_else(|| atributo(e, "w:hAnsi")).unwrap_or_default(),
                    "w:num" => num_id = atributo(e, "w:numId").unwrap_or_default(),
                    "w:abstractNumId" if !num_id.is_empty() => {
                        saida.nums.entry(num_id.clone()).or_default().0 = atributo(e, "w:val").unwrap_or_default();
                    }
                    "w:lvlOverride" if !num_id.is_empty() => sobre = atributo(e, "w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0).min(8),
                    "w:startOverride" if !num_id.is_empty() => {
                        if let Some(v) = atributo(e, "w:val").and_then(|v| v.parse().ok()) {
                            saida.nums.entry(num_id.clone()).or_default().1.insert(sobre, v);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let primeiro_de_titulo = saida.abstratas.values().filter_map(|(niveis, _)| niveis.get(&0)).find(|n| n.estilo.is_some());
    saida.titulos = primeiro_de_titulo.map(|n| if n.def.texto.chars().any(char::is_alphabetic) { Numeracao::Capitulo } else { Numeracao::Decimal });
    saida
}

// ---------------------------------------------------------------------------
// Relações, propriedades, cabeçalho e rodapé, imagens
// ---------------------------------------------------------------------------

/// `rId` → destino (URL do link, arquivo da imagem, parte do cabeçalho).
fn relacoes(xml: &[u8]) -> HashMap<String, String> {
    let mut saida = HashMap::new();
    let mut leitor = Reader::from_reader(xml);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) if e.name().as_ref() == "Relationship" => {
                if let (Some(id), Some(alvo)) = (atributo(e, "Id"), atributo(e, "Target")) {
                    saida.insert(id, alvo);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    saida
}

/// O `w:val` da primeira `tag` (`<w:lang`) do texto.
fn valor_da_tag(xml: &str, tag: &str) -> Option<String> {
    let i = xml.find(tag)?;
    let corpo = xml[i..].split('>').next()?;
    let j = corpo.find("w:val=\"")? + 7;
    Some(corpo[j..].split('"').next()?.to_string())
}

/// As propriedades personalizadas (`docProps/custom.xml`): nome e o texto de cada uma.
fn personalizadas(xml: &[u8]) -> Vec<(String, String)> {
    let mut leitor = Reader::from_reader(xml);
    let (mut v, mut nome, mut dentro) = (Vec::new(), None::<String>, false);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(e) if nome_de(&e) == "property" => nome = atributo(&e, "name"),
            Event::Start(e) if nome.is_some() && nome_de(&e).starts_with("vt:") => {
                dentro = true;
                v.push((nome.clone().unwrap_or_default(), String::new()));
            }
            Event::Text(t) if dentro => {
                if let Some(u) = v.last_mut() {
                    u.1.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) if dentro => {
                if let Some(u) = v.last_mut() {
                    u.1.push_str(&referencia(&r));
                }
            }
            Event::End(e) if e.name().as_ref() == "property" => nome = None,
            Event::End(_) => dentro = false,
            Event::Eof => break,
            _ => {}
        }
    }
    v
}

/// `docProps/core.xml` e `docProps/app.xml`: título, autor e as outras propriedades do arquivo.
fn propriedades(xml: &[u8], doc: &mut Documento) {
    let mut leitor = Reader::from_reader(xml);
    let mut em = String::new();
    let por = |em: &str, v: &str, doc: &mut Documento| {
        let m = &mut doc.extras.documento;
        let campo = match em {
            "dc:title" => &mut doc.meta.titulo,
            "dc:creator" => &mut doc.meta.autor,
            "dc:subject" => &mut m.assunto,
            "cp:keywords" => &mut m.palavras_chave,
            "dc:description" => &mut m.descricao,
            "cp:category" => &mut m.categoria,
            "dcterms:created" => &mut m.criado,
            "dcterms:modified" => &mut m.modificado,
            "Company" => &mut m.empresa,
            "Manager" => &mut m.gerente,
            _ => return,
        };
        campo.push_str(v);
    };
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(e) => em = nome_de(&e),
            Event::Text(t) => por(&em, &t.xml10_content(), doc),
            Event::GeneralRef(r) => por(&em, &referencia(&r), doc),
            Event::End(_) => em.clear(),
            Event::Eof => break,
            _ => {}
        }
    }
}

/// A marca do FeatherDocs para a instrução de um campo do Word (`PAGE` → `{pagina}`).
fn campo_de(instr: &str) -> Option<&'static str> {
    let instr = instr.trim();
    let palavra = instr.split_whitespace().next()?.to_ascii_uppercase();
    if palavra == "PAGE" && instr.to_ascii_lowercase().contains("roman") {
        return Some("{pagina_romano}");
    }
    CAMPOS.iter().find(|(_, i)| i.split_whitespace().next() == Some(palavra.as_str()) && !i.contains("roman")).map(|(marca, _)| *marca)
}

/// Um cabeçalho ou rodapé lido: a faixa e, se o cabeçalho tem, a marca d'água (texto e cor).
struct FaixaLida {
    faixa: Faixas,
    marca: Option<(String, u32)>,
    /// As figuras presas à página que moram no cabeçalho: o timbre, em toda página.
    timbre: Vec<featherdocs_core::modelo::Decoracao>,
}

/// Uma figura `wp:anchor` lida numa faixa, à espera do arquivo: `r:embed`, x, y, largura, altura e atrás.
type AncoraDaFaixa = (String, [f32; 4], bool);

/// Lê a parte `caminho` (`word/header1.xml`) e o logo dela, que mora nas relações da parte.
fn faixa_da_parte(zip: &mut Zip, caminho: &str) -> Option<FaixaLida> {
    let xml = parte(zip, caminho)?;
    let (mut lida, logo, ancoras) = ler_faixa(&xml);
    let (pasta, nome) = caminho.rsplit_once('/').unwrap_or(("word", caminho));
    let rels = parte(zip, &format!("{pasta}/_rels/{nome}.rels")).map(|x| relacoes(&x)).unwrap_or_default();
    let mut imagem = |id: &str| rels.get(id).and_then(|alvo| Some(crate::base64::para_data_url(tipo_da_imagem(alvo), &parte(zip, &caminho_em_word(alvo))?)));
    if let Some((id, w, h)) = logo {
        if let Some(origem) = imagem(&id) {
            lida.faixa.imagem = origem;
            lida.faixa.imagem_largura = w;
            lida.faixa.imagem_altura = h;
        }
    }
    for (id, [x, y, largura, altura], atras) in ancoras {
        if let Some(origem) = imagem(&id) {
            lida.timbre.push(featherdocs_core::modelo::Decoracao { origem, x, y, largura, altura, atras, bloco: None });
        }
    }
    Some(lida)
}

/// Os pedaços de um cabeçalho ou rodapé (separados por tabulação), o alinhamento, a letra do
/// primeiro trecho, o fio e a marca d'água; e o logo (`r:embed`, largura e altura em pt) a resolver.
fn ler_faixa(xml: &[u8]) -> (FaixaLida, Option<(String, f32, f32)>, Vec<AncoraDaFaixa>) {
    let mut leitor = Reader::from_reader(xml);
    let mut pedacos = vec![String::new()];
    let (mut em_t, mut em_instr, mut oculto, mut em_tabs) = (false, false, false, false);
    let mut instr = String::new();
    let mut jc = String::new();
    // A letra vale a do primeiro trecho com texto: `letra` junta o `w:rPr` do trecho aberto.
    let (mut letra, mut letra_lida, mut em_rpr, mut em_pbdr) = ((0u8, 0.0f32, 0u32), None, false, false);
    let mut fio = 0.0;
    let (mut marca, mut cor_marca) = (None, 0u32);
    let (mut logo, mut extensao) = (None, (0.0, 0.0));
    // A figura `wp:anchor` aberta: atrás, a posição (x, y) e o eixo do `wp:posOffset` aberto.
    let (mut ancora, mut ancoras): (Option<(bool, [f32; 2], Option<usize>)>, Vec<AncoraDaFaixa>) = (None, Vec::new());
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let vazio = matches!(ev, Event::Empty(_));
                let nome = nome_de(e);
                if em_rpr {
                    match nome.as_str() {
                        "w:b" if ligado(e) => letra.0 |= featherdocs_core::modelo::NEGRITO,
                        "w:i" if ligado(e) => letra.0 |= featherdocs_core::modelo::ITALICO,
                        "w:rFonts" if atributo(e, "w:ascii").is_some_and(|f| f.contains("Times") || f.contains("Serif") || f.contains("Georgia") || f.contains("Garamond")) => letra.0 |= SERIFA,
                        "w:sz" => letra.1 = numero(e, "w:val").unwrap_or(0.0) / 2.0,
                        "w:color" => letra.2 = atributo(e, "w:val").filter(|v| v != "auto").map_or(0, |v| cor(&v)),
                        _ => {}
                    }
                }
                if em_pbdr && matches!(nome.as_str(), "w:top" | "w:bottom") && atributo(e, "w:val").is_some_and(|v| v != "nil" && v != "none") {
                    fio = numero(e, "w:sz").unwrap_or(4.0) / 8.0;
                }
                match nome.as_str() {
                    "w:r" => letra = (0, 0.0, 0),
                    "w:rPr" => em_rpr = !vazio,
                    "w:pBdr" => em_pbdr = !vazio,
                    "v:shape" => cor_marca = atributo(e, "fillcolor").filter(|c| c.starts_with('#')).map_or(0, |c| (cor(c.trim_start_matches('#')) & 0xffff_ff00) | 0x66),
                    "v:textpath" => marca = atributo(e, "string").filter(|s| !s.is_empty()),
                    "wp:extent" => extensao = (numero(e, "cx").unwrap_or(0.0) / 12700.0, numero(e, "cy").unwrap_or(0.0) / 12700.0),
                    "wp:anchor" => ancora = Some((atributo(e, "behindDoc").is_some_and(|v| v == "1" || v == "true"), [0.0; 2], None)),
                    "wp:positionH" | "wp:positionV" if !vazio => {
                        if let Some(a) = ancora.as_mut() {
                            a.2 = Some(usize::from(nome == "wp:positionV"));
                        }
                    }
                    "a:blip" => match ancora {
                        Some((atras, [x, y], _)) => ancoras.extend(atributo(e, "r:embed").map(|id| (id, [x, y, extensao.0, extensao.1], atras))),
                        None => logo = atributo(e, "r:embed").map(|id| (id, extensao.0, extensao.1)),
                    },
                    _ => {}
                }
                match nome.as_str() {
                    "w:t" => em_t = !vazio,
                    "w:instrText" => em_instr = !vazio,
                    "w:tabs" => em_tabs = !vazio,
                    "w:tab" | "w:ptab" if !em_instr && !em_tabs => pedacos.push(String::new()),
                    "w:jc" => jc = atributo(e, "w:val").unwrap_or_default(),
                    "w:fldSimple" => {
                        if let Some(m) = atributo(e, "w:instr").as_deref().and_then(campo_de) {
                            juntar(&mut pedacos, m);
                        }
                        oculto = !vazio;
                    }
                    "w:fldChar" => match atributo(e, "w:fldCharType").as_deref() {
                        Some("begin") => instr.clear(),
                        Some("separate") => {
                            if let Some(m) = campo_de(&instr) {
                                juntar(&mut pedacos, m);
                            }
                            oculto = true;
                        }
                        Some("end") => oculto = false,
                        _ => {}
                    },
                    _ => {}
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "w:rPr" => em_rpr = false,
                "w:pBdr" => em_pbdr = false,
                "wp:anchor" => ancora = None,
                "wp:positionH" | "wp:positionV" => {
                    if let Some(a) = ancora.as_mut() {
                        a.2 = None;
                    }
                }
                "w:t" => em_t = false,
                "w:instrText" => em_instr = false,
                "w:tabs" => em_tabs = false,
                "w:fldSimple" => oculto = false,
                "w:p" => juntar(&mut pedacos, " "),
                _ => {}
            },
            Event::Text(_) | Event::GeneralRef(_) => {
                let v = match &ev {
                    Event::Text(t) => t.xml10_content().into_owned(),
                    Event::GeneralRef(r) => referencia(r),
                    _ => String::new(),
                };
                if let Some((_, pos, Some(eixo))) = ancora.as_mut() {
                    pos[*eixo] = v.trim().parse::<f32>().unwrap_or(0.0) / 12700.0;
                } else if em_instr {
                    instr.push_str(&v);
                } else if em_t && !oculto {
                    if letra_lida.is_none() && !v.trim().is_empty() {
                        letra_lida = Some(letra);
                    }
                    juntar(&mut pedacos, &v);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let (estilo, tamanho, cor_letra) = letra_lida.unwrap_or_default();
    let faixa = Faixas { estilo, tamanho, cor: cor_letra, fio, ..faixas_de(pedacos.into_iter().map(|p| p.trim().to_string()).collect(), &jc) };
    (FaixaLida { faixa, marca: marca.map(|m| (m, cor_marca)), timbre: Vec::new() }, logo, ancoras)
}

fn juntar(pedacos: &mut [String], s: &str) {
    if let Some(p) = pedacos.last_mut() {
        p.push_str(s);
    }
}

fn faixas_de(mut p: Vec<String>, jc: &str) -> Faixas {
    match p.len() {
        0 => Faixas::default(),
        1 => {
            let t = p.remove(0);
            match jc {
                "center" => Faixas { centro: t, ..Faixas::default() },
                "right" | "end" => Faixas { direita: t, ..Faixas::default() },
                _ => Faixas { esquerda: t, ..Faixas::default() },
            }
        }
        2 => Faixas { esquerda: p.remove(0), centro: p.remove(0), ..Faixas::default() },
        _ => {
            let direita = p[2..].join(" ");
            Faixas { esquerda: p[0].clone(), centro: p[1].clone(), direita, ..Faixas::default() }
        }
    }
}

fn tipo_da_imagem(nome: &str) -> &'static str {
    match nome.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

/// A imagem que mora no `.docx` vira `data:` URL (o documento não depende do arquivo de origem).
fn embutir_imagens(doc: &mut Documento, zip: &mut Zip) {
    for b in &mut doc.blocos {
        let Conteudo::Imagem { origem, .. } = &mut b.conteudo else { continue };
        if origem.is_empty() || origem.contains("://") || origem.starts_with("data:") {
            continue;
        }
        if let Some(bytes) = parte(zip, &caminho_em_word(origem)) {
            *origem = crate::base64::para_data_url(tipo_da_imagem(origem), &bytes);
        }
    }
}

/// Parágrafos de código seguidos viram um bloco só.
fn unir_codigo(doc: &mut Documento) {
    let mut saida: Vec<_> = Vec::with_capacity(doc.blocos.len());
    for b in std::mem::take(&mut doc.blocos) {
        if let (Conteudo::Codigo { codigo: novo, .. }, Some(featherdocs_core::modelo::Bloco { conteudo: Conteudo::Codigo { codigo, .. }, .. })) = (&b.conteudo, saida.last_mut()) {
            codigo.push('\n');
            codigo.push_str(novo);
            continue;
        }
        saida.push(b);
    }
    doc.blocos = saida;
}

// ---------------------------------------------------------------------------
// O corpo do documento
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Par {
    estilo: Option<String>,
    num_id: Option<String>,
    ilvl: u8,
    quebra_antes: bool,
    quebra_depois: bool,
    /// Quebra de coluna no parágrafo (`w:br w:type="column"`).
    coluna: bool,
    /// O parágrafo fecha uma seção (`w:sectPr` no `w:pPr`): onde a seguinte começa.
    secao: Option<InicioDeSecao>,
    trechos: Vec<Trecho>,
    /// As imagens do parágrafo: origem, largura, altura e o texto alternativo.
    imagens: Vec<(String, f32, f32, String)>,
    atrib: Atrib,
    /// A fórmula ocupa o parágrafo (`m:oMathPara`): é um bloco de equação.
    equacao: bool,
    /// O parágrafo trouxe decorações presas à página.
    com_decoracao: bool,
    /// A instrução do campo `TOC` que está neste parágrafo.
    sumario: Option<String>,
    /// A lista gerada (bibliografia, índice) que começa neste parágrafo.
    gerado: Option<Conteudo>,
    /// O parágrafo começou dentro do resultado de uma lista gerada: é jogado fora.
    engolido: bool,
}

#[derive(Default, Clone, Copy)]
struct Corrida {
    estilo: u8,
    fmt: Fmt,
}

#[derive(Default)]
struct AncoraLida {
    atras: bool,
    de: [String; 2],
    pos: [Option<f32>; 2],
    eixo: Option<usize>,
    em_offset: bool,
}

#[derive(Default)]
struct Tabela {
    linhas: Vec<Vec<Celula>>,
    linha: Vec<Celula>,
    celula: Celula,
    cabecalho: bool,
    props: PropsTabela,
    mesclagens: Vec<MesclagemTabela>,
    alinhamento: Alinhamento,
    /// A larguras do `w:tblGrid`, em pontos.
    grade: Vec<f32>,
    /// A célula aberta: o `w:tcPr` dela, o `w:gridSpan` e o `w:vMerge` (`Some(true)` = começa).
    prop_celula: PropsCelula,
    span: usize,
    vmerge: Option<bool>,
    /// Coluna → linha onde a mescla vertical aberta nela começou.
    verticais: HashMap<usize, usize>,
    /// O primeiro parágrafo da célula já passou (o alinhamento do texto sai dele).
    par_na_celula: bool,
}

/// `w:top`, `w:insideH`… de `w:tblBorders`/`w:tcBorders` para o fio do modelo.
fn fio_de_borda(b: &mut BordasDeTabela, nome: &str, e: &BytesStart) {
    let fio = FioDeBorda {
        estilo: atributo(e, "w:val").unwrap_or_else(|| "single".into()),
        espessura: numero(e, "w:sz").map_or(4, |v| v as u16),
        cor: atributo(e, "w:color").map_or(0, |v| cor(&v)),
    };
    let lado = match nome {
        "w:top" => &mut b.cima,
        "w:left" | "w:start" => &mut b.esquerda,
        "w:bottom" => &mut b.baixo,
        "w:right" | "w:end" => &mut b.direita,
        "w:insideH" => &mut b.dentro_h,
        "w:insideV" => &mut b.dentro_v,
        _ => return,
    };
    *lado = Some(fio);
}

/// O campo complexo (`w:fldChar`) em leitura.
#[derive(Default)]
struct Campo {
    instr: String,
    em_instr: bool,
    /// O resultado do campo não vira texto (o sumário e o número da legenda, que o FeatherDocs gera).
    oculto: bool,
    /// O resultado ocupa parágrafos inteiros (bibliografia, índice): eles somem até o fim do campo.
    engolir: bool,
}

struct Leitor {
    estilos: EstilosDocx,
    numeracao: Numeracoes,
    relacoes: HashMap<String, String>,
    doc: Documento,
    pilha: Vec<String>,
    par: Option<Par>,
    corrida: Corrida,
    em_texto: bool,
    em_texto_math: bool,
    math: Option<String>,
    /// O XML do `m:oMath` aberto, remontado dos eventos, para virar LaTeX (`feather_math::omml::para_latex`).
    math_xml: String,
    link: Option<String>,
    tabela: Option<Tabela>,
    prof_tabela: usize,
    sumario_posto: bool,
    /// O tamanho (pt) lido de `wp:extent`, que vem antes do `a:blip` da mesma imagem.
    extensao: (f32, f32),
    /// O `wp:anchor` aberto: atrás do texto, de onde a posição é medida (h, v), a posição em pt e o eixo do `wp:posOffset` aberto.
    ancora: Option<AncoraLida>,
    campo: Campo,
    /// Logo depois do número da legenda: o separador (" – ") some.
    apos_numero: bool,
    cabecalho: Option<String>,
    rodape: Option<String>,
    /// As faixas da primeira página (`w:type="first"`).
    cabecalho_primeira: Option<String>,
    rodape_primeira: Option<String>,
    /// O texto alternativo do desenho aberto (`wp:docPr descr`).
    descricao: String,
    /// Dentro de `w:footnote`/`w:endnote` (em `footnotes.xml`): o `w:id` e quantos blocos havia antes.
    nota: Option<(String, usize)>,
    /// O texto de cada nota lida, pelo `w:id`.
    notas: HashMap<String, Vec<Trecho>>,
    /// A sugestão (`w:ins`, `w:del`) em que o texto está agora.
    rev: Option<Revisao>,
    /// Os `w:id` de comentários que são respostas (as marcas deles no texto não viram comentário).
    respostas: Vec<u64>,
    /// Guarda na sacola o que o modelo não entende (só no corpo: as notas não têm as relações à mão).
    sacola: bool,
    /// As declarações `xmlns:` da raiz da parte, que vão junto com o XML guardado.
    ns: String,
    captura: Option<sacola::Captura>,
    /// Os controles de conteúdo (`w:sdt`) abertos: `Some(bloco)` = guardado, `None` = não.
    controles: Vec<Option<bool>>,
    /// O `w:sdtPr` (e o `w:sdtEndPr`) do controle aberto.
    controle_xml: String,
    /// Os controles de bloco que esperam o próximo parágrafo para pôr a marca de começo.
    controles_pendentes: Vec<String>,
}

impl Leitor {
    fn novo(estilos: EstilosDocx, numeracao: Numeracoes, relacoes: HashMap<String, String>) -> Self {
        Self {
            estilos,
            numeracao,
            relacoes,
            doc: Documento::vazio(),
            pilha: Vec::new(),
            par: None,
            corrida: Corrida::default(),
            em_texto: false,
            em_texto_math: false,
            math: None,
            math_xml: String::new(),
            link: None,
            tabela: None,
            prof_tabela: 0,
            sumario_posto: false,
            extensao: (0.0, 0.0),
            ancora: None,
            campo: Campo::default(),
            apos_numero: false,
            cabecalho: None,
            rodape: None,
            cabecalho_primeira: None,
            rodape_primeira: None,
            descricao: String::new(),
            nota: None,
            notas: HashMap::new(),
            rev: None,
            respostas: Vec::new(),
            sacola: false,
            ns: String::new(),
            captura: None,
            controles: Vec::new(),
            controle_xml: String::new(),
            controles_pendentes: Vec::new(),
        }
    }

    /// Prende um objeto ao parágrafo; `rotulo` guarda o que for preciso para ligá-lo depois.
    fn objeto(&mut self, marca: Marca, rotulo: String) {
        let (estilo, fmt) = (self.corrida.estilo, Fmt { posicao: 0, ..self.corrida.fmt });
        self.com_par(|p| p.trechos.push(Trecho { estilo, fmt, rotulo, ..Trecho::objeto(marca) }));
    }

    /// Fim de `w:footnote`: os parágrafos dela viram o texto da nota (e saem do documento).
    fn fim_da_nota(&mut self) {
        let Some((id, antes)) = self.nota.take() else { return };
        let mut texto: Vec<Trecho> = Vec::new();
        for b in self.doc.blocos.drain(antes.min(self.doc.blocos.len())..) {
            for t in b.conteudo.textos() {
                if !texto.is_empty() {
                    texto.push(Trecho::simples(" "));
                }
                texto.extend(t.iter().cloned());
            }
        }
        if let Some(p) = texto.first_mut() {
            p.texto = p.texto.trim_start().to_string();
        }
        texto.retain(|t| !t.texto.is_empty());
        self.notas.insert(id, texto);
    }

    fn pai(&self) -> &str {
        self.pilha.last().map_or("", String::as_str)
    }

    fn dentro(&self, nome: &str) -> bool {
        self.pilha.iter().any(|p| p == nome)
    }

    fn evento(&mut self, e: Event) {
        if let Some(c) = &mut self.captura {
            c.evento(&e);
        }
        match e {
            Event::Start(e) => {
                self.guardar(&e, false);
                self.abre(&e);
                if self.math.is_some() {
                    self.math_xml.push_str(&etiqueta_math(&e, false));
                }
                self.pilha.push(nome_de(&e));
            }
            Event::Empty(e) => {
                self.guardar(&e, true);
                self.abre(&e);
                if self.math.is_some() {
                    self.math_xml.push_str(&etiqueta_math(&e, true));
                }
                self.fecha(&nome_de(&e));
                self.fim_da_captura();
            }
            Event::End(e) => {
                self.pilha.pop();
                if self.math.is_some() {
                    let q = e.name();
                    let nome: &str = q.as_ref();
                    self.math_xml.push_str(&format!("</{nome}>"));
                }
                self.fecha(e.name().as_ref());
                self.fim_da_captura();
            }
            Event::Text(t) => {
                let v = t.xml10_content().into_owned();
                if self.math.is_some() && self.em_texto_math {
                    self.math_xml.push_str(&esc_math(&v));
                }
                self.texto(&v);
            }
            Event::GeneralRef(r) => {
                let v = referencia(&r);
                if self.math.is_some() && self.em_texto_math {
                    self.math_xml.push_str(&esc_math(&v));
                }
                self.texto(&v);
            }
            _ => {}
        }
    }

    /// O elemento que começa vai para a sacola (ou já está dentro de um que vai).
    fn guardar(&mut self, e: &BytesStart, vazio: bool) {
        if self.pilha.is_empty() {
            self.ns = sacola::declaracoes(e);
        }
        if let Some(c) = &mut self.captura {
            c.abre(e, vazio);
            return;
        }
        let comeca = match e.name().as_ref() {
            "w:drawing" | "w:object" => self.sacola && self.par.is_some(),
            "w:sdtPr" | "w:sdtEndPr" => matches!(self.controles.last(), Some(Some(_))),
            _ => false,
        };
        if comeca {
            self.captura = Some(sacola::Captura::nova(e, vazio, &self.ns, self.pilha.len()));
        }
    }

    /// O elemento guardado terminou: vira objeto do Word no texto, ou o `w:sdtPr` do controle aberto.
    fn fim_da_captura(&mut self) {
        if !self.captura.as_ref().is_some_and(|c| c.prof == self.pilha.len()) {
            return;
        }
        let Some(c) = self.captura.take() else { return };
        if c.raiz.starts_with("w:sdt") {
            // shortcut: o controle que embrulha sumário, bibliografia ou capa (docPartObj) não é guardado,
            // porque o FeatherDocs gera esse conteúdo; guardar quando o sumário for um controle de verdade.
            let xml = c.xml();
            if xml.contains("w:docPartObj") {
                if let Some(t) = self.controles.last_mut() {
                    *t = None;
                }
            } else {
                self.controle_xml.push_str(&xml);
            }
        } else if let Some(resumo) = c.resumo() {
            self.objeto(Marca::ObjetoWord { xml: c.xml(), partes: Vec::new(), resumo }, String::new());
        }
    }

    /// `w:sdt`, `w:sdtContent` e o fim do `w:sdt`: as marcas de começo e de fim do controle.
    fn controle(&mut self, nome: &str) {
        let marca = |xml: String, fim: bool, bloco: bool| Trecho::objeto(Marca::Controle { xml, fim, bloco });
        match nome {
            "w:sdt" => {
                // shortcut: o controle de bloco dentro de tabela (de linha, de célula) não é guardado;
                // entra quando a célula tiver blocos (fase 4).
                let guarda = self.sacola && (self.par.is_some() || self.tabela.is_none());
                self.controles.push(guarda.then_some(self.par.is_none()));
                self.controle_xml.clear();
            }
            "w:sdtContent" => match self.controles.last() {
                Some(Some(true)) => self.controles_pendentes.push(std::mem::take(&mut self.controle_xml)),
                Some(Some(false)) => {
                    let m = marca(std::mem::take(&mut self.controle_xml), false, false);
                    self.com_par(|p| p.trechos.push(m));
                }
                _ => {}
            },
            _ => match self.controles.pop() {
                Some(Some(false)) => self.com_par(|p| p.trechos.push(marca(String::new(), true, false))),
                // O controle de bloco vazio não deixa marca; o que teve parágrafo fecha no último bloco.
                Some(Some(true)) if self.controles_pendentes.pop().is_none() => {
                    if let Some(t) = self.doc.blocos.last_mut().and_then(|b| b.conteudo.textos_mut().pop()) {
                        t.push(marca(String::new(), true, true));
                    }
                }
                _ => {}
            },
        }
    }

    fn texto(&mut self, t: &str) {
        if let Some(a) = self.ancora.as_mut().filter(|a| a.em_offset) {
            if let (Some(k), Ok(v)) = (a.eixo, t.trim().parse::<f32>()) {
                a.pos[k] = Some(v / 12700.0);
            }
            return;
        }
        if self.campo.em_instr {
            self.campo.instr.push_str(t);
        } else if self.em_texto_math {
            if let Some(m) = &mut self.math {
                m.push_str(t);
            }
        } else if self.em_texto && !self.campo.oculto {
            self.empurrar(t);
        }
    }

    fn empurrar(&mut self, t: &str) {
        let t = if self.apos_numero {
            let resto = t.trim_start_matches([' ', '\u{2013}', '\u{2014}', '-', ':', '.']);
            if resto.is_empty() {
                return;
            }
            self.apos_numero = false;
            resto
        } else {
            t
        };
        let estilo = self.corrida.estilo | if self.math.is_some() { MATEMATICA } else { 0 };
        let fmt = self.corrida.fmt;
        let link = self.link.clone();
        let rev = self.rev.clone().map(Box::new);
        let Some(p) = &mut self.par else { return };
        if let Some(u) = p.trechos.last_mut() {
            if u.marca.is_none() && u.estilo == estilo && u.link == link && u.fmt == fmt && u.rev == rev {
                u.texto.push_str(t);
                return;
            }
        }
        p.trechos.push(Trecho { texto: t.to_string(), estilo, link, fmt, rev, ..Trecho::default() });
    }

    fn abre(&mut self, e: &BytesStart) {
        let nome = nome_de(e);
        if self.prop_de_tabela(&nome, e) {
            return;
        }
        match nome.as_str() {
            "w:p" => {
                let trechos = self.controles_pendentes.drain(..).map(|xml| Trecho::objeto(Marca::Controle { xml, fim: false, bloco: true })).collect();
                self.par = Some(Par { engolido: self.campo.engolir, trechos, atrib: self.estilos.atrib_normal, ..Par::default() });
            }
            "w:sdt" | "w:sdtContent" => self.controle(&nome),
            "w:r" => self.corrida = Corrida::default(),
            "w:t" | "w:delText" => self.em_texto = true,
            "m:t" => self.em_texto_math = true,
            "w:ins" | "w:del" if self.pai() != "w:rPr" && self.pai() != "w:pPr" => {
                let tipo = if nome == "w:ins" { TipoDeRevisao::Insercao } else { TipoDeRevisao::Remocao };
                let quando = crate::docx_comentarios::data_de_iso(&atributo(e, "w:date").unwrap_or_default());
                self.rev = Some(Revisao { tipo, autor: atributo(e, "w:author").unwrap_or_default(), quando });
            }
            "w:commentRangeStart" | "w:commentRangeEnd" => {
                let id: u64 = atributo(e, "w:id").and_then(|v| v.parse().ok()).unwrap_or(0);
                if !self.respostas.contains(&id) {
                    self.objeto(Marca::Comentario { id, fim: nome == "w:commentRangeEnd" }, String::new());
                }
            }
            "m:oMath" => {
                self.math = Some(String::new());
                self.math_xml.clear();
            }
            "m:oMathPara" => self.com_par(|p| p.equacao = true),
            "w:sectPr" if self.pai() == "w:pPr" => self.com_par(|p| p.secao = Some(InicioDeSecao::ProximaPagina)),
            "w:tbl" => self.abre_tabela(),
            "w:tr" => {
                if let Some(t) = &mut self.tabela {
                    t.linha.clear();
                }
            }
            "w:tc" => {
                if let (Some(t), 1) = (&mut self.tabela, self.prof_tabela) {
                    t.celula.clear();
                    t.prop_celula = PropsCelula::default();
                    t.span = 1;
                    t.vmerge = None;
                    t.par_na_celula = false;
                }
            }
            "w:tblHeader" => {
                if let Some(t) = &mut self.tabela {
                    t.cabecalho = t.linhas.is_empty();
                }
            }
            "w:background" => self.doc.extras.cor_pagina = atributo(e, "w:color").map(|v| cor(&v)).unwrap_or(0),
            "w:fldSimple" => self.campo_simples(e),
            "w:fldChar" => self.campo_complexo(e),
            "w:instrText" => self.campo.em_instr = true,
            "w:footnote" | "w:endnote" => self.nota = atributo(e, "w:id").map(|id| (id, self.doc.blocos.len())),
            "w:footnoteReference" | "w:endnoteReference" => {
                let marca = Marca::Nota { fim: nome == "w:endnoteReference", texto: Vec::new() };
                self.objeto(marca, atributo(e, "w:id").unwrap_or_default());
            }
            _ => self.propriedade(&nome, e),
        }
    }

    /// As propriedades da tabela de fora (`w:tblPr`, `w:tblGrid`, `w:trPr`, `w:tcPr`); `false` se `nome` não é uma delas.
    fn prop_de_tabela(&mut self, nome: &str, e: &BytesStart) -> bool {
        if self.prof_tabela != 1 {
            return false;
        }
        let pai = self.pilha.last().cloned().unwrap_or_default();
        let Some(t) = &mut self.tabela else { return false };
        let pt = |a: &str| twips_para_pt(e, a);
        match (pai.as_str(), nome) {
            ("w:tblGrid", "w:gridCol") => t.grade.push(pt("w:w").unwrap_or(0.0)),
            ("w:tblPr", "w:tblW") if atributo(e, "w:type").as_deref() == Some("pct") => {
                let v = atributo(e, "w:w").unwrap_or_default();
                // O Word grava em cinquentésimos de ponto percentual; o ISO aceita também "50%".
                t.props.largura_pct = match v.strip_suffix('%') {
                    Some(p) => p.parse().unwrap_or(0.0),
                    None => v.parse::<f32>().map_or(0.0, |n| n / 50.0),
                };
            }
            ("w:tblPr", "w:jc") => {
                t.alinhamento = match atributo(e, "w:val").as_deref() {
                    Some("center") => Alinhamento::Centro,
                    Some("right" | "end") => Alinhamento::Direita,
                    _ => Alinhamento::Esquerda,
                };
            }
            ("w:tblPr", "w:tblLayout") => t.props.fixa = atributo(e, "w:type").as_deref() == Some("fixed"),
            ("w:tblBorders", _) => fio_de_borda(&mut t.props.bordas, nome, e),
            ("w:tcBorders", _) => fio_de_borda(&mut t.prop_celula.bordas, nome, e),
            ("w:trPr", "w:trHeight") => {
                let altura = pt("w:val").unwrap_or(0.0);
                if altura > 0.0 {
                    let exata = atributo(e, "w:hRule").as_deref() == Some("exact");
                    t.props.alturas.push(AlturaDeLinha { linha: t.linhas.len(), altura, exata });
                }
            }
            ("w:tcPr", "w:gridSpan") => t.span = numero(e, "w:val").map_or(1, |n| (n as usize).clamp(1, 63)),
            ("w:tcPr", "w:vMerge") => t.vmerge = Some(atributo(e, "w:val").as_deref() == Some("restart")),
            ("w:tcPr", "w:shd") => {
                let fundo = atributo(e, "w:fill").map_or(0, |v| cor(&v));
                // shortcut: o cinza que o FeatherDocs grava no cabeçalho volta como "sem fundo" (a tela já o desenha);
                // some se o cabeçalho ganhar fundo próprio no modelo.
                let do_cabecalho = t.cabecalho && t.linhas.is_empty() && fundo == 0xF2F2F2FF;
                if !do_cabecalho {
                    t.prop_celula.fundo = fundo;
                }
            }
            ("w:tcPr", "w:vAlign") => {
                t.prop_celula.vertical = match atributo(e, "w:val").as_deref() {
                    Some("center") => 1,
                    Some("bottom") => 2,
                    _ => 0,
                };
            }
            ("w:tcPr", "w:textDirection") => t.prop_celula.direcao = atributo(e, "w:val").unwrap_or_default(),
            ("w:tcMar", _) => {
                let m = t.prop_celula.margens.get_or_insert([0.0; 4]);
                let i = match nome {
                    "w:top" => 0,
                    "w:left" | "w:start" => 1,
                    "w:bottom" => 2,
                    "w:right" | "w:end" => 3,
                    _ => return true,
                };
                m[i] = pt("w:w").unwrap_or(0.0);
            }
            ("w:tcPr", "w:tcW") | ("w:tblPr", _) | ("w:tblGrid", _) | ("w:tcPr", _) => {}
            _ => return false,
        }
        true
    }

    fn abre_tabela(&mut self) {
        self.prof_tabela += 1;
        if self.prof_tabela == 1 {
            self.tabela = Some(Tabela::default());
        }
    }

    /// O campo que o FeatherDocs gera sozinho: o sumário e o número da legenda.
    fn campo_gerado(&mut self, instr: &str) -> bool {
        let palavras: Vec<&str> = instr.split_whitespace().collect();
        match palavras.first().map(|p| p.to_ascii_uppercase()).as_deref() {
            Some("TOC") => {
                let instr = instr.to_string();
                self.com_par(|p| p.sumario = Some(instr));
                true
            }
            Some("BIBLIOGRAPHY") | Some("INDEX") => {
                let c = if palavras[0].eq_ignore_ascii_case("INDEX") { Conteudo::IndiceRemissivo } else { Conteudo::Bibliografia { todas: instr.contains("\\f") } };
                self.com_par(|p| p.gerado = Some(c));
                self.campo.engolir = true;
                true
            }
            Some("SEQ") => {
                let rotulo = palavras.get(1).copied().unwrap_or("").trim_matches('"').to_lowercase();
                let tipo = ROTULOS_LEGENDA.iter().position(|r| r.to_lowercase() == rotulo).or(match rotulo.as_str() {
                    "figure" => Some(0),
                    "table" => Some(1),
                    "equation" => Some(2),
                    _ => None,
                });
                self.com_par(|p| {
                    p.atrib.legenda = tipo.map_or(1, |t| t as u8 + 1);
                    p.trechos.clear();
                });
                self.apos_numero = true;
                true
            }
            _ => false,
        }
    }

    fn campo_simples(&mut self, e: &BytesStart) {
        let instr = atributo(e, "w:instr").unwrap_or_default();
        if self.campo_gerado(&instr) || self.campo_objeto(&instr) {
            self.campo.oculto = true;
        }
    }

    /// O campo que vira objeto preso ao texto (o resultado que o Word guardou não entra: o motor refaz).
    fn campo_objeto(&mut self, instr: &str) -> bool {
        let Some(m) = objeto_do_campo(instr) else { return false };
        self.objeto(m, String::new());
        true
    }

    fn campo_complexo(&mut self, e: &BytesStart) {
        match atributo(e, "w:fldCharType").as_deref() {
            Some("begin") => self.campo.instr.clear(),
            Some("separate") => {
                let instr = std::mem::take(&mut self.campo.instr);
                if self.campo_gerado(&instr) || self.campo_objeto(&instr) {
                    self.campo.oculto = true;
                }
            }
            Some("end") => {
                self.campo.engolir = false;
                // Campo sem resultado (o `XE` do índice): a instrução só termina aqui.
                let instr = std::mem::take(&mut self.campo.instr);
                if !instr.trim().is_empty() {
                    self.campo_objeto(&instr);
                }
                self.campo.oculto = false;
            }
            _ => {}
        }
    }

    /// Propriedades de parágrafo, de corrida e de seção, e os elementos soltos (tab, br, link, imagem).
    fn propriedade(&mut self, nome: &str, e: &BytesStart) {
        let pai = self.pai().to_string();
        if pai == "w:rPr" && self.dentro("w:r") {
            if nome == "w:rStyle" {
                self.estilo_de_caractere(e);
            }
            prop_caractere(nome, e, &mut self.corrida.estilo, &mut self.corrida.fmt, &self.estilos.fontes_do_tema);
            return;
        }
        if pai == "w:pPr" && self.par.is_some() {
            if let Some(p) = &mut self.par {
                prop_paragrafo(nome, e, &mut p.atrib);
            }
        } else if pai == "w:pBdr" {
            if let Some(p) = &mut self.par {
                borda_do_paragrafo(nome, e, &mut p.atrib);
            }
            return;
        } else if pai == "w:tabs" {
            if let Some(p) = &mut self.par {
                if nome == "w:tab" {
                    tabulacao(e, &mut p.atrib);
                }
            }
            return;
        } else if pai == "w:sectPr" || pai == "w:pgBorders" || (matches!(pai.as_str(), "w:footnotePr" | "w:endnotePr") && self.dentro("w:sectPr")) {
            self.secao(nome, e, &pai);
            return;
        }
        self.solto(nome, e);
    }

    /// O `w:rStyle`: a letra do estilo de caractere (com a herança) entra no trecho, por baixo da
    /// formatação direta que vem depois dele no `w:rPr`, e o trecho guarda o id para a volta.
    // shortcut: a letra fica copiada no trecho (mudar o estilo depois não muda o texto); passar para
    // o motor resolver na composição quando houver como editar estilo de caractere.
    fn estilo_de_caractere(&mut self, e: &BytesStart) {
        let Some(id) = atributo(e, "w:val").and_then(|v| self.estilos.ids.get(&v).copied()) else { return };
        let Some(r) = self.estilos.modelo.resolvido(id).filter(|r| r.caractere) else { return };
        let (bits, f) = r.caractere_do_estilo();
        let c = &mut self.corrida;
        if !f.familia.vazia() || bits & (SERIFA | CODIGO) != 0 {
            c.estilo &= !(SERIFA | CODIGO);
            c.fmt.familia = f.familia;
        }
        c.estilo |= bits;
        for (de, para) in [(f.tamanho, &mut c.fmt.tamanho)] {
            if de != 0.0 {
                *para = de;
            }
        }
        for (de, para) in [(f.cor, &mut c.fmt.cor), (f.realce, &mut c.fmt.realce)] {
            if de != 0 {
                *para = de;
            }
        }
        if f.posicao != 0 {
            c.fmt.posicao = f.posicao;
        }
        c.fmt.mais.somar(&f.mais, 0);
        c.fmt.estilo_de_caractere = id;
    }

    fn solto(&mut self, nome: &str, e: &BytesStart) {
        match nome {
            "w:pStyle" => {
                let id = atributo(e, "w:val");
                // O parágrafo começa com o que o estilo do Word (com a herança e o padrão) define, e o
                // `w:pPr` direto vem por cima. O do FeatherDocs já foi gravado carimbado no parágrafo.
                let estilo = id.as_ref().and_then(|v| self.estilos.ids.get(v)).and_then(|n| self.estilos.modelo.resolvido(*n)).filter(|r| !r.caractere);
                self.com_par(|p| {
                    match &estilo {
                        Some(r) if !r.codigo.is_empty() => p.atrib = Atrib { estilo: r.id, ..r.paragrafo_do_estilo() },
                        Some(r) => p.atrib.estilo = r.id,
                        None => {}
                    }
                    p.estilo = id;
                });
            }
            "w:numId" => self.com_par(|p| p.num_id = atributo(e, "w:val")),
            "w:ilvl" => self.com_par(|p| p.ilvl = atributo(e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(0)),
            "w:tab" if self.pai() == "w:r" && self.par.is_some() => self.empurrar("\t"),
            "w:br" => self.quebra_de_linha(e),
            // O indicador marca o bloco onde está (o que vai sair a seguir); os ocultos do Word (`_Toc…`, `_GoBack`) ficam de fora.
            "w:bookmarkStart" if self.nota.is_none() => {
                if let Some(nome) = atributo(e, "w:name").filter(|n| !n.starts_with('_')) {
                    let bloco = self.doc.proximo_id;
                    self.doc.marcadores.push(featherdocs_core::modelo::Marcador { nome, bloco });
                }
            }
            "w:hyperlink" => self.link = atributo(e, "r:id").and_then(|id| self.relacoes.get(&id).cloned()).or_else(|| atributo(e, "w:anchor").map(|a| format!("#{a}"))),
            "a:blip" => {
                let origem = atributo(e, "r:embed").or_else(|| atributo(e, "r:link")).and_then(|id| self.relacoes.get(&id).cloned()).unwrap_or_default();
                let (w, h) = std::mem::take(&mut self.extensao);
                // Presa à página (ou à margem): vira decoração ancorada no bloco deste parágrafo; o resto, imagem em linha.
                let pagina = self.doc.pagina;
                let posicao = self.ancora.as_ref().and_then(|a| {
                    let eixo = |k: usize, margem: f32| match a.de[k].as_str() {
                        "page" => a.pos[k],
                        "margin" => a.pos[k].map(|v| v + margem),
                        _ => None,
                    };
                    Some((eixo(0, pagina.margem_esq)?, eixo(1, pagina.margem_sup)?, a.atras))
                });
                match posicao {
                    Some((x, y, atras)) if w > 0.0 && h > 0.0 && self.tabela.is_none() => {
                        let bloco = Some(self.doc.proximo_id);
                        self.doc.extras.decoracoes.push(featherdocs_core::modelo::Decoracao { origem, x, y, largura: w, altura: h, atras, bloco });
                        self.com_par(|p| p.com_decoracao = true);
                    }
                    _ => {
                        let descricao = std::mem::take(&mut self.descricao);
                        self.com_par(|p| p.imagens.push((origem, w, h, descricao)));
                    }
                }
            }
            "wp:docPr" => self.descricao = atributo(e, "descr").unwrap_or_default(),
            "wp:anchor" => self.ancora = Some(AncoraLida { atras: atributo(e, "behindDoc").as_deref() == Some("1"), ..AncoraLida::default() }),
            "wp:positionH" | "wp:positionV" => {
                if let Some(a) = &mut self.ancora {
                    let k = usize::from(nome == "wp:positionV");
                    a.eixo = Some(k);
                    a.de[k] = atributo(e, "relativeFrom").unwrap_or_default();
                }
            }
            "wp:posOffset" => {
                if let Some(a) = &mut self.ancora {
                    a.em_offset = true;
                }
            }
            "wp:extent" => {
                let pt = |n: &str| numero(e, n).unwrap_or(0.0) / 12700.0;
                self.extensao = (pt("cx"), pt("cy"));
            }
            _ => {}
        }
    }

    /// `w:sectPr`: papel, margens, borda, números de linha, alinhamento vertical, cabeçalho e rodapé.
    fn secao(&mut self, nome: &str, e: &BytesStart, pai: &str) {
        if (pai, nome) == ("w:sectPr", "w:type") && self.dentro("w:pPr") {
            let inicio = match atributo(e, "w:val").as_deref() {
                Some("continuous") => InicioDeSecao::Continua,
                Some("evenPage") => InicioDeSecao::PaginaPar,
                Some("oddPage") => InicioDeSecao::PaginaImpar,
                _ => InicioDeSecao::ProximaPagina,
            };
            self.com_par(|p| p.secao = Some(inicio));
            return;
        }
        let x = &mut self.doc.extras;
        let p = &mut self.doc.pagina;
        match (pai, nome) {
            ("w:sectPr", "w:pgSz") => {
                p.largura = twips_para_pt(e, "w:w").unwrap_or(p.largura);
                p.altura = twips_para_pt(e, "w:h").unwrap_or(p.altura);
            }
            ("w:sectPr", "w:pgMar") => {
                p.margem_sup = twips_para_pt(e, "w:top").map_or(p.margem_sup, f32::abs);
                p.margem_dir = twips_para_pt(e, "w:right").unwrap_or(p.margem_dir);
                p.margem_inf = twips_para_pt(e, "w:bottom").map_or(p.margem_inf, f32::abs);
                p.margem_esq = twips_para_pt(e, "w:left").unwrap_or(p.margem_esq);
                // A posição do cabeçalho e do rodapé é a do Word, também a padrão (708 = 1,25 cm).
                x.documento.distancia_cabecalho = twips_para_pt(e, "w:header").unwrap_or(0.0).max(0.0);
                x.documento.distancia_rodape = twips_para_pt(e, "w:footer").unwrap_or(0.0).max(0.0);
            }
            ("w:sectPr", "w:vAlign") => {
                x.vertical = match atributo(e, "w:val").as_deref() {
                    Some("center") => Vertical::Centro,
                    Some("bottom") => Vertical::Base,
                    _ => Vertical::Topo,
                }
            }
            ("w:sectPr", "w:lnNumType") => x.numerar_linhas = if atributo(e, "w:restart").as_deref() == Some("newPage") { NumeracaoDeLinhas::PorPagina } else { NumeracaoDeLinhas::Continua },
            ("w:sectPr", "w:titlePg") => x.primeira_diferente = ligado(e),
            ("w:sectPr", "w:pgNumType") => {
                if let Some(n) = atributo(e, "w:start").and_then(|v| v.parse().ok()) {
                    x.pagina_inicial = n;
                }
                x.documento.formato_pagina = atributo(e, "w:fmt").filter(|f| f != "decimal").unwrap_or_default();
            }
            ("w:footnotePr", "w:numFmt") => x.formato_notas = formato_da_nota(atributo(e, "w:val").as_deref(), 0),
            ("w:endnotePr", "w:numFmt") => x.formato_notas_fim = formato_da_nota(atributo(e, "w:val").as_deref(), 2),
            ("w:sectPr", "w:headerReference") if atributo(e, "w:type").as_deref() == Some("first") => self.cabecalho_primeira = atributo(e, "r:id"),
            ("w:sectPr", "w:footerReference") if atributo(e, "w:type").as_deref() == Some("first") => self.rodape_primeira = atributo(e, "r:id"),
            ("w:sectPr", "w:headerReference") if atributo(e, "w:type").as_deref() != Some("first") && atributo(e, "w:type").as_deref() != Some("even") => self.cabecalho = atributo(e, "r:id"),
            ("w:sectPr", "w:footerReference") if atributo(e, "w:type").as_deref() != Some("first") && atributo(e, "w:type").as_deref() != Some("even") => self.rodape = atributo(e, "r:id"),
            ("w:pgBorders", "w:top") => {
                x.borda_cor = atributo(e, "w:color").map(|v| cor(&v)).filter(|c| *c != 0).unwrap_or(0xFF);
                x.borda_espessura = numero(e, "w:sz").unwrap_or(4.0) / 8.0;
                x.borda_distancia = numero(e, "w:space").unwrap_or(0.0);
            }
            _ => {}
        }
    }

    fn com_par(&mut self, f: impl FnOnce(&mut Par)) {
        if let Some(p) = &mut self.par {
            f(p);
        }
    }

    fn estilo_do_par(&self) -> Option<&EstiloDocx> {
        self.par.as_ref().and_then(|p| p.estilo.as_ref()).and_then(|id| self.estilos.get(id))
    }

    fn quebra_de_linha(&mut self, e: &BytesStart) {
        if atributo(e, "w:type").as_deref() == Some("page") {
            let tem_texto = self.par.as_ref().is_some_and(|p| !p.trechos.is_empty());
            self.com_par(|p| {
                if tem_texto {
                    p.quebra_depois = true;
                } else {
                    p.quebra_antes = true;
                }
            });
        } else if atributo(e, "w:type").as_deref() == Some("column") {
            self.com_par(|p| p.coluna = true);
        } else if self.par.is_some() {
            let codigo = self.estilo_do_par().is_some_and(|s| s.nome.contains("code") || s.nome.contains("preformatted"));
            let linha = QUEBRA_DE_LINHA.to_string();
            self.empurrar(if codigo { "\n" } else { &linha });
        }
    }

    fn fecha(&mut self, nome: &str) {
        match nome {
            "w:footnote" | "w:endnote" => self.fim_da_nota(),
            "w:p" => self.fim_do_paragrafo(),
            "w:sdt" => self.controle("/w:sdt"),
            "w:t" | "w:delText" => self.em_texto = false,
            "m:t" => self.em_texto_math = false,
            "w:ins" | "w:del" if self.pai() != "w:rPr" && self.pai() != "w:pPr" => self.rev = None,
            "w:instrText" => self.campo.em_instr = false,
            "w:fldSimple" => self.campo.oculto = false,
            "w:hyperlink" => self.link = None,
            "wp:posOffset" => {
                if let Some(a) = &mut self.ancora {
                    a.em_offset = false;
                }
            }
            "wp:anchor" => self.ancora = None,
            "m:oMath" => {
                if let Some(m) = self.math.take() {
                    // A estrutura do OMML vira LaTeX (fração, raiz, ∑…); sem ela, fica o texto corrido.
                    let latex = feather_math::omml::para_latex(&std::mem::take(&mut self.math_xml));
                    let m = if latex.is_empty() { m.trim().to_string() } else { latex };
                    self.corrida = Corrida::default();
                    self.math = Some(String::new());
                    self.empurrar(&m);
                    self.math = None;
                }
            }
            "w:tc" => self.fim_da_celula(),
            "w:tr" => {
                if let Some(t) = &mut self.tabela {
                    let l = std::mem::take(&mut t.linha);
                    t.linhas.push(l);
                }
            }
            "w:tbl" => self.fim_da_tabela(),
            _ => {}
        }
    }

    fn fim_da_celula(&mut self) {
        if self.prof_tabela != 1 {
            return;
        }
        if let Some(t) = &mut self.tabela {
            let (linha, coluna) = (t.linhas.len(), t.linha.len());
            let c = std::mem::take(&mut t.celula);
            t.linha.push(c);
            let mut props = std::mem::take(&mut t.prop_celula);
            match t.vmerge {
                Some(true) => {
                    t.verticais.insert(coluna, linha);
                    props.linhas = 1;
                }
                Some(false) => {
                    if let Some(&ancora) = t.verticais.get(&coluna) {
                        t.props.celula_mut(ancora, coluna).linhas += 1;
                    }
                }
                None => {
                    t.verticais.remove(&coluna);
                }
            }
            // A célula encoberta pela de cima não guarda nada próprio.
            if t.vmerge != Some(false) && props != PropsCelula::default() {
                props.linha = linha;
                props.coluna = coluna;
                t.props.celulas.push(props);
            }
            // Com gridSpan, o modelo guarda as células encobertas (vazias) e a mesclagem.
            if t.span > 1 {
                t.linha.extend(std::iter::repeat_with(Celula::new).take(t.span - 1));
                t.mesclagens.push(MesclagemTabela { linha, coluna, colunas: t.span });
            }
        }
    }

    fn fim_da_tabela(&mut self) {
        self.prof_tabela = self.prof_tabela.saturating_sub(1);
        if self.prof_tabela == 0 {
            if let Some(mut t) = self.tabela.take() {
                if !t.linhas.is_empty() {
                    let colunas = t.linhas.iter().map(Vec::len).max().unwrap_or(0);
                    if t.grade.len() == colunas && t.grade.iter().all(|w| *w > 0.0) {
                        t.props.colunas = t.grade;
                    }
                    t.props.celulas.retain(|c| *c != PropsCelula { linha: c.linha, coluna: c.coluna, ..PropsCelula::default() });
                    // A grade cinza que o FeatherDocs grava quando a tabela não tem bordas próprias volta como "sem bordas".
                    let cinza = Some(FioDeBorda { estilo: "single".into(), espessura: 4, cor: 0xBFBFBFFF });
                    let b = &t.props.bordas;
                    if [&b.cima, &b.esquerda, &b.baixo, &b.direita, &b.dentro_h, &b.dentro_v].iter().all(|f| **f == cinza) {
                        t.props.bordas = BordasDeTabela::default();
                    }
                    self.doc.acrescentar(Conteudo::Tabela { cabecalho: t.cabecalho, linhas: t.linhas });
                    if let Some(b) = self.doc.blocos.last_mut() {
                        b.tabela = t.props;
                        b.mesclagens = t.mesclagens;
                        b.atrib.alinhamento = t.alinhamento;
                    }
                }
            }
        }
    }

    fn fim_do_paragrafo(&mut self) {
        self.apos_numero = false;
        let Some(p) = self.par.take() else { return };
        if let Some(t) = &mut self.tabela {
            if self.prof_tabela == 1 && !t.par_na_celula {
                t.par_na_celula = true;
                if p.atrib.alinhamento != Alinhamento::Esquerda {
                    t.prop_celula.alinhamento = p.atrib.alinhamento;
                }
            }
            if !t.celula.is_empty() && !p.trechos.is_empty() {
                t.celula.push(Trecho::simples(" "));
            }
            t.celula.extend(p.trechos);
            return;
        }
        // O parágrafo que só leva decorações (o de 1 pt que o FeatherDocs grava antes do bloco) não vira
        // bloco: elas ficam ancoradas no seguinte, que recebe o mesmo id.
        // shortcut: o parágrafo vazio de verdade que só tinha a figura também some (perde uma linha em branco).
        if p.com_decoracao && p.trechos.iter().all(|t| t.texto.trim().is_empty()) && p.imagens.is_empty() {
            return;
        }
        self.emitir(p);
    }

    fn acrescentar(&mut self, c: Conteudo, atrib: Atrib) {
        self.doc.acrescentar(c);
        if let Some(b) = self.doc.blocos.last_mut() {
            b.atrib = atrib;
        }
    }

    fn emitir(&mut self, p: Par) {
        let estilo = p.estilo.as_ref().and_then(|id| self.estilos.get(id)).cloned();
        let nome = estilo.as_ref().map_or("", |e| e.nome.as_str()).to_string();
        if p.quebra_antes {
            self.doc.acrescentar(Conteudo::QuebraDePagina);
        }
        let vazio = p.trechos.iter().all(|t| t.texto.trim().is_empty());
        if let Some(c) = p.gerado {
            self.doc.acrescentar(c);
        } else if p.engolido && p.trechos.iter().all(|t| t.texto.trim().is_empty()) || p.engolido && self.campo.engolir {
            // Um parágrafo do resultado da lista gerada.
        } else if let Some(instr) = &p.sumario {
            self.sumario_posto = true;
            self.doc.acrescentar(sumario_de(instr));
        } else if nome == "toc heading" {
            // O título do sumário é gerado pelo próprio sumário.
        } else if nome.starts_with("toc ") || nome == "table of contents" {
            if !self.sumario_posto {
                self.sumario_posto = true;
                self.doc.acrescentar(Conteudo::sumario(3));
            }
        } else if p.estilo.as_deref() == Some("FDLegendaImagem") {
            self.legenda_da_imagem(p.trechos.clone());
        } else if p.equacao {
            let latex: String = p.trechos.iter().map(|t| t.texto.as_str()).collect();
            self.acrescentar(Conteudo::Equacao { latex: latex.trim().to_string() }, p.atrib);
        } else if vazio && (p.secao.is_some() || p.coluna) && p.trechos.is_empty() {
            // Parágrafo vazio que só carrega a quebra: ela entra sozinha, logo abaixo.
        } else if vazio && p.imagens.is_empty() && !p.quebra_antes && !p.quebra_depois {
            if p.atrib.borda == 8 && p.trechos.is_empty() {
                self.doc.acrescentar(Conteudo::Regua);
            } else {
                // O parágrafo vazio com estilo de título fica parágrafo, e sem o estilo (que não vale nele).
                let mut atrib = p.atrib;
                if estilo.as_ref().is_some_and(|e| e.nivel.is_some()) {
                    atrib.estilo = 0;
                }
                self.acrescentar(Conteudo::Paragrafo { texto: p.trechos.clone() }, atrib);
            }
        } else if !vazio {
            let (c, atrib) = self.bloco_de_texto(&p, estilo.as_ref());
            self.acrescentar(c, atrib);
        }
        for (origem, largura, altura, descricao) in p.imagens {
            self.doc.acrescentar(Conteudo::Imagem { origem, legenda: Vec::new(), largura, altura });
            if let Some(b) = self.doc.blocos.last_mut() {
                b.imagem.descricao = descricao;
            }
        }
        if p.quebra_depois {
            self.doc.acrescentar(Conteudo::QuebraDePagina);
        }
        if p.coluna {
            self.doc.acrescentar(Conteudo::QuebraDeColuna);
        }
        if let Some(inicio) = p.secao {
            self.doc.acrescentar(Conteudo::QuebraDeSecao { inicio });
        }
    }

    fn legenda_da_imagem(&mut self, texto: Vec<Trecho>) {
        if let Some(Conteudo::Imagem { legenda, .. }) = self.doc.blocos.last_mut().map(|b| &mut b.conteudo) {
            *legenda = texto;
        } else {
            self.doc.acrescentar(Conteudo::Paragrafo { texto });
        }
    }

    fn bloco_de_texto(&mut self, p: &Par, estilo: Option<&EstiloDocx>) -> (Conteudo, Atrib) {
        let texto = p.trechos.clone();
        let nome = estilo.map_or("", |e| e.nome.as_str());
        let mut atrib = p.atrib;
        // O `w:outlineLvl` direto, sem estilo de título, fica no parágrafo (`MaisParagrafo::nivel_topico`).
        if let Some(nivel) = estilo.and_then(|e| e.nivel) {
            atrib.sem_numero = p.num_id.as_deref() == Some("0");
            return (Conteudo::Titulo { nivel, texto }, atrib);
        }
        // A lista: a do `w:numPr` direto (o `numId` 0 tira a do estilo), ou a do estilo (List Number).
        let do_estilo = (atrib.mais.lista, atrib.mais.nivel_lista);
        (atrib.mais.lista, atrib.mais.nivel_lista) = (0, 0);
        let lista = match p.num_id.as_deref() {
            Some("0") => None,
            Some(num) => self.numeracao.lista_do_modelo(num, false).map(|l| (l, p.ilvl, Some(num))),
            None => (do_estilo.0 != 0).then_some((do_estilo.0, do_estilo.1, None)),
        };
        if let Some((l, ilvl, num)) = lista {
            let niveis = match usize::from(l).checked_sub(1) {
                Some(i) => self.numeracao.modelo[i].niveis.clone(),
                None => num.map(|n| self.numeracao.niveis(&self.numeracao.nums[n].0)).unwrap_or_default(),
            };
            let tipo = tipo_da_lista(&niveis, ilvl);
            atrib.lista = tipo.lista;
            atrib.marcado = tipo.marcado;
            atrib.mais.lista = l;
            // O `startOverride` no primeiro item do `numId`: recomeça (no FeatherDocs, o item logo depois de
            // um parágrafo já recomeça sozinho) ou começa noutro número.
            if let Some(v) = num.and_then(|n| self.numeracao.reinicio(n, ilvl)) {
                let inicio = niveis.get(usize::from(ilvl)).map_or(1, |n| n.inicio);
                if v != if l == 0 { 1 } else { inicio } {
                    atrib.mais.inicio = Some(v);
                } else {
                    atrib.reiniciar = l != 0 || matches!(self.doc.blocos.last().map(|b| &b.conteudo), Some(Conteudo::Item { .. }));
                }
            }
            // O `w:ind` do item conta da margem; no FeatherDocs o recuo é o que passa do nível.
            let recuo = niveis.get(usize::from(ilvl)).filter(|n| l != 0 && n.recuo > 0.0).map_or(crate::docx_gravar::RECUO_DO_NIVEL * f32::from(ilvl + 1), |n| n.recuo);
            atrib.recuo_esq = (atrib.recuo_esq - recuo).max(0.0);
            return (Conteudo::Item { ordenado: tipo.numerada, nivel: ilvl.min(5), texto }, atrib);
        }
        if nome.contains("quote") || nome.contains("citação") {
            return (Conteudo::Citacao { texto }, atrib);
        }
        if nome.contains("code") || nome.contains("preformatted") || p.trechos.iter().all(|t| t.estilo & CODIGO != 0) {
            let codigo = featherdocs_core::inline::texto_puro(&texto);
            return (Conteudo::Codigo { linguagem: String::new(), codigo }, Atrib::default());
        }
        (Conteudo::Paragrafo { texto }, atrib)
    }

    /// Os estilos (com nome, o padrão e o tema) e a numeração dos títulos voltam ao documento.
    fn estilos_do_documento(&mut self) {
        let numeracao = self.doc.estilos.numeracao;
        self.doc.estilos = Estilos { numeracao, listas: std::mem::take(&mut self.numeracao.modelo), ..self.estilos.modelo.clone() };
        let titulos_numerados = self.estilos.mapa.values().any(|s| s.numerado && nivel_do_nome(&s.nome).is_some());
        self.doc.estilos.numeracao = if titulos_numerados { self.numeracao.titulos.unwrap_or(Numeracao::Decimal) } else { Numeracao::Nenhuma };
    }
}

/// O sumário de uma instrução `TOC`: `\o "1-3"` (níveis), `\c "Figura"` (lista de figuras), `\n` (sem números).
fn sumario_de(instr: &str) -> Conteudo {
    let niveis = instr.split("\\o").nth(1).and_then(|r| r.trim().trim_start_matches('"').split(['-', '"']).nth(1)).and_then(|n| n.trim().parse::<u8>().ok()).unwrap_or(3).clamp(1, 6);
    let rotulo = instr.split("\\c").nth(1).map(|r| r.trim().trim_start_matches('"').split('"').next().unwrap_or("").to_lowercase());
    let lista = match rotulo.as_deref() {
        Some(r) if r == ROTULOS_LEGENDA[0].to_lowercase() || r == "figure" => 1,
        Some(r) if r == ROTULOS_LEGENDA[1].to_lowercase() || r == "table" => 2,
        _ => 0,
    };
    let sem_numeros = instr.split_whitespace().any(|p| p == "\\n");
    Conteudo::Sumario { niveis, sem_numeros, sem_pontilhado: false, lista, latex: false }
}

pub(crate) mod mapa;
pub(crate) mod sacola;

#[cfg(test)]
mod testes;
