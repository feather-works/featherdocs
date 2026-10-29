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

use featherdocs_core::modelo::{classe_da_familia, Secao, AlturaDeLinha, BordasDeTabela, FioDeBorda, MesclagemTabela, PropsCelula, PropsTabela, Atrib, Celula, ConfigPagina, Conteudo, Documento, Estilo, Estilos, Faixas, Familia, Fmt, InicioDeSecao, Lista, ListaDoWord, Marca, NivelDeLista, Numeracao, NumeracaoDeLinhas, Revisao, TipoDeRevisao, Trecho, Vertical, Alinhamento, MaisLetra, MaisParagrafo, Tabulacao, ALINHAMENTO_DEFINIDO, CODIGO, DISTRIBUIDO, ITALICO, NEGRITO, QUEBRA_DE_LINHA, MATEMATICA, SERIFA, SUBLINHADO, TACHADO};
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
        l.doc.extras.mais.hifenizar = s.find("<w:autoHyphenation").is_some_and(|i| {
            let tag = s[i..].split('>').next().unwrap_or("");
            !tag.contains("w:val=\"0\"") && !tag.contains("w:val=\"false\"")
        });
        // 708 (1,25 cm) é o de sempre do Word e o que o FeatherDocs grava.
        l.doc.extras.mais.tabulacao_padrao = s.find("<w:defaultTabStop").and_then(|i| s[i..].split("w:val=\"").nth(1)).and_then(|v| v.split('"').next()).and_then(|v| v.parse::<f32>().ok()).filter(|v| *v != 708.0).map_or(0.0, |v| v / 20.0);
        l.doc.extras.protegido = s.find("<w:documentProtection").is_some_and(|i| {
            let tag = s[i..].split('>').next().unwrap_or("");
            tag.contains("readOnly") && (tag.contains("w:enforcement=\"1\"") || tag.contains("w:enforcement=\"true\"") || tag.contains("w:enforcement=\"on\""))
        });
    }
    let pares = parte(&mut zip, "word/settings.xml").is_some_and(|x| String::from_utf8_lossy(&x).find("<w:evenAndOddHeaders").is_some_and(|i| {
        let tag = String::from_utf8_lossy(&x)[i..].split('>').next().unwrap_or("").to_string();
        !tag.contains("w:val=\"0\"") && !tag.contains("w:val=\"false\"")
    }));
    let mut faixa = |rel: &Option<String>| rel.as_ref().and_then(|id| l.relacoes.get(id).cloned()).and_then(|alvo| faixa_do_pacote(&mut zip, &caminho_em_word(&alvo), &l.estilos)).unwrap_or_default();
    // A seção sem cabeçalho próprio herda o da de antes (como no Word); a última é a do documento.
    let mut faixas_das_secoes = Vec::new();
    let mut antes: [Option<String>; 2] = [None, None];
    for (id, rids) in &l.secoes {
        for k in 0..2 {
            if rids[k].is_some() {
                antes[k] = rids[k].clone();
            }
        }
        faixas_das_secoes.push((*id, faixa(&antes[0]), faixa(&antes[1])));
    }
    for k in 0..2 {
        let lista = if k == 0 { &mut l.cabecalhos } else { &mut l.rodapes };
        if lista[0].is_none() {
            lista[0] = antes[k].clone();
        }
    }
    let [cab, cab1, cab2] = [0, 1, 2].map(|k| faixa(&l.cabecalhos[k]));
    let [rod, rod1, rod2] = [0, 1, 2].map(|k| faixa(&l.rodapes[k]));
    let marca = l.cabecalhos[0].as_ref().and_then(|id| l.relacoes.get(id)).and_then(|alvo| parte(&mut zip, &caminho_em_word(alvo))).and_then(|x| marca_dagua(&String::from_utf8_lossy(&x)));
    let mut doc = l.doc;
    for (id, cabecalho, rodape) in faixas_das_secoes {
        if let Some(s) = doc.blocos.iter_mut().find(|b| b.id == id).and_then(|b| b.secao.as_mut()) {
            (s.cabecalho, s.rodape) = (cabecalho, rodape);
        }
    }
    doc.pagina = papel_conhecido(doc.pagina);
    let x = &mut doc.extras;
    (x.cabecalho, x.rodape, x.cabecalho_primeira, x.rodape_primeira) = (cab, rod, cab1, rod1);
    x.mais.pares_diferentes = pares;
    if let Some((texto, cor)) = marca {
        (x.marca, x.marca_cor) = (texto, cor);
    }
    if pares {
        (x.mais.cabecalho_par, x.mais.rodape_par) = (cab2, rod2);
    }
    // A seção igual à do documento (o FeatherDocs grava todas iguais) não guarda nada próprio.
    let (pagina, cabecalho, rodape) = (papel_conhecido(doc.pagina), doc.extras.cabecalho.clone(), doc.extras.rodape.clone());
    for b in &mut doc.blocos {
        if let Some(s) = &mut b.secao {
            s.pagina = papel_conhecido(s.pagina);
            if s.pagina == pagina && s.cabecalho == cabecalho && s.rodape == rodape {
                b.secao = None;
            }
        }
    }
    unir_codigo(&mut doc);
    embutir_imagens(&mut doc, &mut zip);
    sacola::embutir(&mut doc, &mut zip);
    for nome in ["docProps/core.xml", "docProps/app.xml", "docProps/custom.xml"] {
        if let Some(x) = parte(&mut zip, nome) {
            propriedades(&x, &mut doc);
        }
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
    featherdocs_core::modelo::cada_bloco_mut(&mut doc.blocos, &mut |b| {
        for t in b.conteudo.textos_mut() {
            for tr in t.iter_mut() {
                if let Some(Marca::Nota { fim, texto }) = tr.marca.as_deref_mut() {
                    *texto = notas.get(&(*fim, tr.rotulo.clone())).cloned().unwrap_or_default();
                    tr.rotulo.clear();
                }
            }
        }
    });
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
        "DATE" | "CREATEDATE" | "SAVEDATE" => {
            let formato = instr.split("\\@").nth(1).map(|f| f.trim().split('"').nth(1).unwrap_or(f.trim()).to_string()).unwrap_or_else(|| "dd/MM/yyyy".into());
            Marca::Campo { campo: featherdocs_core::campo::data_com_formato(&formato) }
        }
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
/// Um atributo liga/desliga da DrawingML (`behindDoc="1"`, `flipH="true"`).
fn ligado_attr(e: &BytesStart, nome: &str) -> bool {
    atributo(e, nome).is_some_and(|v| v == "1" || v == "true")
}

/// Uma medida do CSS da VML em pt (`100pt`, `2in`, `3cm`, `50px`).
fn medida_css(v: &str) -> Option<f32> {
    let v = v.trim();
    let (n, fator) = [("pt", 1.0), ("in", 72.0), ("cm", 72.0 / 2.54), ("mm", 72.0 / 25.4), ("px", 0.75)].iter().find_map(|(u, f)| v.strip_suffix(u).map(|n| (n, *f))).unwrap_or((v, 1.0));
    n.trim().parse::<f32>().ok().map(|n| n * fator)
}

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
    let mut lidos = xml.map(|x| estilos(x, &fontes_do_tema)).unwrap_or_default();
    // O português é o idioma que o gravador põe quando o documento não diz outro: não entra no padrão.
    if lidos.padrao.fmt.mais.idioma.nome() == "pt-BR" {
        lidos.padrao.fmt.mais.idioma = Familia::default();
    }
    numeracao.estilos_de_lista = lidos.mapa.iter().filter(|(_, s)| s.tipo == "numbering").filter_map(|(id, s)| Some((id.clone(), s.num_id.clone()?))).collect();
    let mut embutidos = estilos(crate::docx_gravar::estilos_embutidos().as_bytes(), &(String::new(), String::new()));
    embutidos.padrao.fmt.mais.idioma = Familia::default();
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
    let de_tabela = xml.map(estilos_de_tabela).unwrap_or_default();
    let mut modelo = Estilos { lista, de_tabela, ..modelo };
    if !lidos.padrao.mesma_aparencia(&embutidos.padrao) {
        modelo.padrao.por_caractere((lidos.padrao.bits, lidos.padrao.fmt));
        modelo.padrao.por_paragrafo(&lidos.padrao.atrib);
        modelo.padrao.desligados = lidos.padrao.desligados;
    }
    let atrib_normal = modelo.resolvido_do_bloco(&featherdocs_core::modelo::Bloco::novo(0, Conteudo::Paragrafo { texto: Vec::new() })).map(|e| e.paragrafo_do_estilo()).unwrap_or_default();
    EstilosDocx { mapa: lidos.mapa, ids, modelo, fontes_do_tema, atrib_normal }
}

/// Os `w:style w:type="table"` do `styles.xml`, como vieram (os estilos não se aninham).
fn estilos_de_tabela(xml: &[u8]) -> String {
    let s = String::from_utf8_lossy(xml);
    let mut saida = String::new();
    let mut resto = s.as_ref();
    while let Some(i) = resto.find("<w:style ") {
        let tag = resto[i..].split('>').next().unwrap_or("");
        let fim = if tag.ends_with('/') { Some(i + tag.len() + 1) } else { resto[i..].find("</w:style>").map(|f| i + f + "</w:style>".len()) };
        let Some(fim) = fim else { break };
        if tag.contains("w:type=\"table\"") {
            saida.push_str(&resto[i..fim]);
        }
        resto = &resto[fim..];
    }
    saida
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

/// As propriedades de `docProps/core.xml`, `app.xml` (empresa, gerente) e `custom.xml` (as personalizadas).
fn propriedades(xml: &[u8], doc: &mut Documento) {
    let mut leitor = Reader::from_reader(xml);
    let mut em = String::new();
    // Em `custom.xml`, o nome da propriedade aberta (o valor vem no filho `vt:…`).
    let mut propriedade: Option<String> = None;
    while let Ok(ev) = leitor.read_event() {
        let v = match ev {
            Event::Start(e) => {
                em = nome_de(&e);
                if em == "property" {
                    propriedade = atributo(&e, "name");
                    if let Some(n) = &propriedade {
                        doc.meta.mais.personalizadas.push((n.clone(), String::new()));
                    }
                }
                continue;
            }
            Event::Text(t) => t.xml10_content().into_owned(),
            Event::GeneralRef(r) => referencia(&r),
            Event::End(e) => {
                if e.name().as_ref() == "property" {
                    propriedade = None;
                }
                em.clear();
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let m = &mut doc.meta;
        let alvo = match em.as_str() {
            "dc:title" => &mut m.titulo,
            "dc:creator" => &mut m.autor,
            "dc:subject" => &mut m.mais.assunto,
            "cp:keywords" => &mut m.mais.palavras_chave,
            "dc:description" => &mut m.mais.descricao,
            "dcterms:created" => &mut m.mais.criado,
            "dcterms:modified" => &mut m.mais.modificado,
            "Company" => &mut m.mais.empresa,
            "Manager" => &mut m.mais.gerente,
            n if n.starts_with("vt:") && propriedade.is_some() => match m.mais.personalizadas.last_mut() {
                Some(p) => &mut p.1,
                None => continue,
            },
            _ => continue,
        };
        alvo.push_str(&v);
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

/// Os pedaços de um cabeçalho ou rodapé (separados por tabulação) e o alinhamento.
/// A marca d'água de texto do Word no cabeçalho: o `v:textpath` (WordArt da VML) e a cor dele.
fn marca_dagua(xml: &str) -> Option<(String, u32)> {
    let valor = |tag: &str, nome: &str| tag.split(&format!(" {nome}=\"")).nth(1).and_then(|r| r.split('"').next()).map(str::to_string);
    // O primeiro `v:textpath` com texto (o da definição do tipo de forma, `v:shapetype`, não tem).
    let i = xml.match_indices("<v:textpath").map(|(i, _)| i).find(|&i| valor(xml[i..].split('>').next().unwrap_or(""), "string").is_some())?;
    let tag = xml[i..].split('>').next()?;
    let texto = valor(tag, "string")?.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
    let inicio = xml[..i].rfind("<v:shape ");
    let forma = inicio.map(|k| xml[k..].split('>').next().unwrap_or(""));
    let rgb = match forma.and_then(|f| valor(f, "fillcolor")).as_deref().map(|c| c.trim_start_matches('#').to_ascii_lowercase()) {
        Some(c) if c.len() == 6 && c.chars().all(|x| x.is_ascii_hexdigit()) => u32::from_str_radix(&c, 16).unwrap_or(0xC0C0C0),
        Some(c) if c.starts_with("gray") || c.starts_with("grey") => 0x808080,
        Some(c) if c == "black" => 0,
        Some(c) if c == "red" => 0xFF0000,
        Some(c) if c == "blue" => 0x0000FF,
        _ => 0xC0C0C0,
    };
    // A opacidade (`v:fill opacity=".5"`) vira o alfa; sem ela, opaca.
    let opacidade = inicio.and_then(|k| xml[k..i].find("<v:fill").map(|f| k + f)).and_then(|f| valor(xml[f..].split('>').next().unwrap_or(""), "opacity"));
    let alfa = opacidade.and_then(|o| o.trim_end_matches('f').parse::<f32>().ok()).map_or(0xFF, |o| (if o > 1.0 { o / 65536.0 } else { o }.clamp(0.0, 1.0) * 255.0).round() as u32);
    Some((texto, rgb << 8 | alfa))
}

/// O papel (`w:pgSz`) ou as margens (`w:pgMar`) de um `w:sectPr`.
fn papel_do_sectpr(p: &mut ConfigPagina, nome: &str, e: &BytesStart) {
    if nome == "w:pgSz" {
        p.largura = twips_para_pt(e, "w:w").unwrap_or(p.largura);
        p.altura = twips_para_pt(e, "w:h").unwrap_or(p.altura);
    } else {
        p.margem_sup = twips_para_pt(e, "w:top").map_or(p.margem_sup, f32::abs);
        p.margem_dir = twips_para_pt(e, "w:right").unwrap_or(p.margem_dir);
        p.margem_inf = twips_para_pt(e, "w:bottom").map_or(p.margem_inf, f32::abs);
        p.margem_esq = twips_para_pt(e, "w:left").unwrap_or(p.margem_esq);
    }
}

/// O cabeçalho ou rodapé na parte `caminho` do pacote, com a imagem dele embutida.
fn faixa_do_pacote(zip: &mut Zip, caminho: &str, estilos: &EstilosDocx) -> Option<Faixas> {
    let xml = parte(zip, caminho)?;
    let (pasta, nome) = caminho.rsplit_once('/').unwrap_or(("", caminho));
    let rels = parte(zip, &format!("{pasta}/_rels/{nome}.rels")).map(|x| relacoes(&x)).unwrap_or_default();
    let mut f = ler_faixa(&xml);
    // O que não cabe nas três posições (tabela, vários parágrafos) fica também em blocos, lidos como o corpo.
    let mut l = Leitor::novo(estilos.clone(), Numeracoes::default(), rels.clone());
    let mut leitor = Reader::from_reader(xml.as_slice());
    while let Ok(e) = leitor.read_event() {
        if matches!(e, Event::Eof) {
            break;
        }
        l.evento(e);
    }
    let com_texto = l.doc.blocos.iter().filter(|b| matches!(&b.conteudo, Conteudo::Paragrafo { texto } if texto.iter().any(|t| !t.texto.trim().is_empty()))).count();
    if com_texto > 1 || l.doc.blocos.iter().any(|b| !matches!(b.conteudo, Conteudo::Paragrafo { .. } | Conteudo::Imagem { .. })) {
        embutir_imagens(&mut l.doc, zip);
        f.blocos = l.doc.blocos;
    }
    if let Some(alvo) = rels.get(&f.imagem) {
        f.imagem = match parte(zip, &caminho_em_word(alvo)) {
            Some(bytes) => crate::base64::para_data_url(tipo_da_imagem(alvo), &bytes),
            None => String::new(),
        };
    } else {
        f.imagem.clear();
    }
    if !f.blocos.is_empty() {
        f.lida = f.assinatura();
    }
    Some(f)
}

/// O texto da faixa nas três posições, a letra do primeiro texto e a primeira imagem (`imagem` = o `r:embed` dela).
fn ler_faixa(xml: &[u8]) -> Faixas {
    let mut leitor = Reader::from_reader(xml);
    let mut pedacos = vec![String::new()];
    let (mut em_t, mut em_instr, mut oculto, mut em_tabs) = (false, false, false, false);
    let mut instr = String::new();
    let mut jc = String::new();
    // A letra da corrida aberta (bits, tamanho, cor) e a do primeiro texto, que vale para a faixa.
    let (mut corrida, mut letra) = ((0u8, 0.0f32, 0u32), None);
    let (mut imagem, mut extensao) = (String::new(), (0.0, 0.0));
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let vazio = matches!(ev, Event::Empty(_));
                match nome_de(e).as_str() {
                    "w:r" => corrida = (0, 0.0, 0),
                    "w:b" if ligado(e) => corrida.0 |= NEGRITO,
                    "w:i" if ligado(e) => corrida.0 |= ITALICO,
                    "w:u" if atributo(e, "w:val").is_some_and(|v| v != "none") => corrida.0 |= SUBLINHADO,
                    "w:sz" => corrida.1 = numero(e, "w:val").unwrap_or(0.0) / 2.0,
                    "w:color" => corrida.2 = atributo(e, "w:color").or_else(|| atributo(e, "w:val")).filter(|c| c != "auto").map_or(0, |c| cor(&c)),
                    "wp:extent" if imagem.is_empty() => extensao = (numero(e, "cx").unwrap_or(0.0) / 12700.0, numero(e, "cy").unwrap_or(0.0) / 12700.0),
                    "a:blip" if imagem.is_empty() => imagem = atributo(e, "r:embed").unwrap_or_default(),
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
                if em_instr {
                    instr.push_str(&v);
                } else if em_t && !oculto {
                    if letra.is_none() && !v.trim().is_empty() {
                        letra = Some(corrida);
                    }
                    juntar(&mut pedacos, &v);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let (estilo, tamanho, cor) = letra.unwrap_or_default();
    Faixas { estilo, tamanho, cor, imagem, imagem_largura: extensao.0, imagem_altura: extensao.1, ..faixas_de(pedacos.into_iter().map(|p| p.trim().to_string()).collect(), &jc) }
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
    let mut embutir = |origem: &mut String| {
        if origem.is_empty() || origem.contains("://") || origem.starts_with("data:") {
            return;
        }
        if let Some(bytes) = parte(zip, &caminho_em_word(origem)) {
            *origem = crate::base64::para_data_url(tipo_da_imagem(origem), &bytes);
        }
    };
    featherdocs_core::modelo::cada_bloco_mut(&mut doc.blocos, &mut |b| {
        if let Conteudo::Imagem { origem, .. } = &mut b.conteudo {
            embutir(origem);
        }
        // A imagem de dentro da célula, que o texto da célula mostra no meio da linha.
        for t in b.conteudo.textos_mut() {
            for tr in t.iter_mut() {
                if let Some(Marca::Imagem { origem, .. }) = tr.marca.as_deref_mut() {
                    embutir(origem);
                }
            }
        }
    });
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
    /// O papel da seção que fecha aqui e o `r:id` do cabeçalho e do rodapé dela.
    papel_da_secao: Option<ConfigPagina>,
    faixas_da_secao: [Option<String>; 2],
    trechos: Vec<Trecho>,
    imagens: Vec<ImagemLida>,
    atrib: Atrib,
    /// A fórmula ocupa o parágrafo (`m:oMathPara`): é um bloco de equação.
    equacao: bool,
    /// A instrução do campo `TOC` que está neste parágrafo.
    sumario: Option<String>,
    /// A lista gerada (bibliografia, índice) que começa neste parágrafo.
    gerado: Option<Conteudo>,
    /// O parágrafo começou dentro do resultado de uma lista gerada: é jogado fora.
    engolido: bool,
}

/// Uma imagem do parágrafo: `posicao` = em linha, no trecho de índice tal (a flutuante não tem).
#[derive(Default)]
struct ImagemLida {
    origem: String,
    largura: f32,
    altura: f32,
    props: featherdocs_core::modelo::PropsImagem,
    posicao: Option<usize>,
}

#[derive(Default, Clone, Copy)]
struct Corrida {
    estilo: u8,
    fmt: Fmt,
}

#[derive(Default)]
struct Tabela {
    linhas: Vec<Vec<Celula>>,
    linha: Vec<Celula>,
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
    /// Os blocos de fora, guardados enquanto a célula aberta junta os dela em `doc.blocos`.
    fora: Vec<featherdocs_core::modelo::Bloco>,
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
    /// O campo que só vai e volta: a instrução e quantos trechos o parágrafo tinha quando o resultado começou.
    guardado: Option<(String, usize)>,
    /// O campo `HYPERLINK`: o resultado é o texto do link.
    link: bool,
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
    /// As tabelas abertas, a de fora primeiro (a tabela dentro de uma célula vem depois).
    tabelas: Vec<Tabela>,
    sumario_posto: bool,
    /// O tamanho (pt) lido de `wp:extent`, que vem antes do `a:blip` da mesma imagem.
    extensao: (f32, f32),
    /// A imagem do `w:drawing` (ou do `w:pict` antigo) aberto, juntando as propriedades até ele fechar.
    desenho: Option<ImagemLida>,
    /// Qual posição do `wp:anchor` está aberta: 1 = horizontal, 2 = vertical.
    posicao_aberta: u8,
    /// Dentro do `w:txbxContent` da caixa de texto: o parágrafo e os blocos de fora, guardados.
    caixa_aberta: Option<(Option<Par>, Vec<featherdocs_core::modelo::Bloco>)>,
    /// Os blocos da caixa que acabou de fechar, à espera do fim do `w:drawing`.
    caixa_lida: Option<Vec<featherdocs_core::modelo::Bloco>>,
    /// O `mc:Choice` da `mc:AlternateContent` aberta teve caixa de texto: o `mc:Fallback` (a mesma caixa em VML) é pulado.
    choice_com_caixa: bool,
    /// Pulando o `mc:Fallback`: a profundidade dele na pilha.
    pulando: Option<usize>,
    campo: Campo,
    /// Logo depois do número da legenda: o separador (" – ") some.
    apos_numero: bool,
    /// O `r:id` do cabeçalho e do rodapé de cada tipo: o de sempre, o da primeira página e o das pares.
    cabecalhos: [Option<String>; 3],
    rodapes: [Option<String>; 3],
    /// Os rótulos de legenda próprios (`SEQ Quadro`), na ordem em que aparecem.
    legendas: Vec<String>,
    /// Os indicadores (`w:bookmarkStart`) à espera do próximo bloco do corpo.
    indicadores: Vec<String>,
    /// As quebras de seção com seção própria: o id do bloco e o `r:id` do cabeçalho e do rodapé dela.
    secoes: Vec<(u64, [Option<String>; 2])>,
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
            tabelas: Vec::new(),
            sumario_posto: false,
            extensao: (0.0, 0.0),
            desenho: None,
            posicao_aberta: 0,
            caixa_aberta: None,
            caixa_lida: None,
            choice_com_caixa: false,
            pulando: None,
            campo: Campo::default(),
            apos_numero: false,
            cabecalhos: Default::default(),
            rodapes: Default::default(),
            secoes: Vec::new(),
            indicadores: Vec::new(),
            legendas: Vec::new(),
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
                // O parágrafo seguinte da nota começa depois de um QUEBRA_DE_PARAGRAFO no fim do texto de antes.
                match texto.last_mut() {
                    Some(u) if u.marca.is_none() => u.texto.push(featherdocs_core::modelo::QUEBRA_DE_PARAGRAFO),
                    Some(_) => texto.push(Trecho::simples(&featherdocs_core::modelo::QUEBRA_DE_PARAGRAFO.to_string())),
                    None => {}
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
        if let Some(prof) = self.pulando {
            match &e {
                Event::Start(e) => self.pilha.push(nome_de(e)),
                Event::End(_) => {
                    self.pilha.pop();
                    if self.pilha.len() < prof {
                        self.pulando = None;
                    }
                }
                _ => {}
            }
            return;
        }
        match &e {
            Event::Start(x) if x.name().as_ref() == "mc:AlternateContent" => self.choice_com_caixa = false,
            Event::Start(x) if x.name().as_ref() == "mc:Fallback" && self.choice_com_caixa => {
                self.pilha.push(nome_de(x));
                self.pulando = Some(self.pilha.len());
                return;
            }
            Event::Start(x) if x.name().as_ref() == "w:txbxContent" && self.caixa_aberta.is_none() && self.captura.is_some() => {
                self.choice_com_caixa = self.dentro("mc:Choice");
                self.caixa_aberta = Some((self.par.take(), std::mem::take(&mut self.doc.blocos)));
            }
            Event::End(x) if x.name().as_ref() == "w:txbxContent" => {
                if let Some((par, fora)) = self.caixa_aberta.take() {
                    self.caixa_lida = Some(std::mem::replace(&mut self.doc.blocos, fora));
                    self.par = par;
                }
            }
            _ => {}
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
            let mut xml = c.xml();
            let blocos = self.caixa_lida.take().unwrap_or_default();
            // O texto da caixa fica nos blocos; no XML guardado, só o lugar dele.
            if let (Some(i), Some(f)) = (xml.find("<w:txbxContent>"), xml.rfind("</w:txbxContent>")) {
                let i = i + "<w:txbxContent>".len();
                if i <= f {
                    xml.replace_range(i..f, featherdocs_core::modelo::TEXTO_DA_CAIXA);
                }
            }
            self.objeto(Marca::ObjetoWord { xml, partes: Vec::new(), resumo, blocos }, String::new());
        }
    }

    /// `w:sdt`, `w:sdtContent` e o fim do `w:sdt`: as marcas de começo e de fim do controle.
    fn controle(&mut self, nome: &str) {
        let marca = |xml: String, fim: bool, bloco: bool| Trecho::objeto(Marca::Controle { xml, fim, bloco });
        match nome {
            "w:sdt" => {
                // shortcut: o controle de bloco dentro de tabela (de linha, de célula) não é guardado;
                // a célula já tem blocos: guardar quando o fim do controle souber voltar para a célula certa.
                let guarda = self.sacola && (self.par.is_some() || self.tabelas.is_empty());
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
        let pai = self.pai().to_string();
        if pai == "wp:posOffset" || pai == "wp:align" {
            if let Some(p) = self.posicao_flutuante() {
                if pai == "wp:align" {
                    p.alinhar = t.trim().to_string();
                } else {
                    p.deslocamento = t.trim().parse::<f32>().unwrap_or(0.0) / 12700.0;
                }
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
            let resto = t.trim_start_matches([' ', '\u{2013}', '\u{2014}', '-', ':', '.', ')']);
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
            "w:ins" | "w:del" | "w:moveTo" | "w:moveFrom" if self.pai() != "w:rPr" && self.pai() != "w:pPr" => {
                let tipo = if matches!(nome.as_str(), "w:ins" | "w:moveTo") { TipoDeRevisao::Insercao } else { TipoDeRevisao::Remocao };
                let quando = crate::docx_comentarios::data_de_iso(&atributo(e, "w:date").unwrap_or_default());
                self.rev = Some(Revisao { tipo, autor: atributo(e, "w:author").unwrap_or_default(), quando, movido: nome.starts_with("w:move") });
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
            "w:sectPr" if self.pai() == "w:pPr" => {
                let papel = self.doc.pagina;
                self.com_par(|p| (p.secao, p.papel_da_secao) = (Some(InicioDeSecao::ProximaPagina), Some(papel)));
            }
            "w:tbl" => self.abre_tabela(),
            "w:tr" => {
                if let Some(t) = self.tabelas.last_mut() {
                    t.linha.clear();
                }
            }
            "w:tc" => {
                if let Some(t) = self.tabelas.last_mut() {
                    t.prop_celula = PropsCelula::default();
                    t.span = 1;
                    t.vmerge = None;
                    t.par_na_celula = false;
                    t.fora = std::mem::take(&mut self.doc.blocos);
                }
            }
            "w:tblHeader" => {
                if let Some(t) = self.tabelas.last_mut() {
                    t.cabecalho = t.linhas.is_empty();
                }
            }
            "w:background" => self.doc.extras.cor_pagina = atributo(e, "w:color").map(|v| cor(&v)).unwrap_or(0),
            "w:fldSimple" => self.campo_simples(e),
            "w:fldChar" => self.campo_complexo(e),
            "w:instrText" => self.campo.em_instr = true,
            // O indicador entra no bloco do parágrafo em que começa (o `_GoBack` e os do sumário, que o Word refaz, não).
            "w:bookmarkStart" => {
                if let Some(n) = atributo(e, "w:name").filter(|n| n != "_GoBack" && !n.starts_with("_Toc")) {
                    self.indicadores.push(n);
                }
            }
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
        let pai = self.pilha.last().cloned().unwrap_or_default();
        let Some(t) = self.tabelas.last_mut() else { return false };
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
            ("w:tblPr", "w:tblStyle") => t.props.estilo = atributo(e, "w:val").unwrap_or_default(),
            ("w:tblPr", "w:tblLook") => t.props.aparencia = e.attributes_raw().trim().to_string(),
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
        self.tabelas.push(Tabela::default());
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
                // O rótulo próprio (`SEQ Quadro`) entra na lista do documento: legenda 4 em diante.
                let original = palavras.get(1).copied().unwrap_or("").trim_matches('"').to_string();
                let proprio = (tipo.is_none() && !original.is_empty()).then(|| match self.legendas.iter().position(|r| r.to_lowercase() == rotulo) {
                    Some(i) => i,
                    None => {
                        self.legendas.push(original);
                        self.legendas.len() - 1
                    }
                }).filter(|i| *i < 200);
                self.com_par(|p| {
                    p.atrib.legenda = match (tipo, proprio) {
                        (Some(t), _) => t as u8 + 1,
                        (None, Some(i)) => i as u8 + 4,
                        _ => 1,
                    };
                    // Na equação numerada (a fórmula e o número entre parênteses), a fórmula fica.
                    if tipo == Some(2) {
                        p.trechos.retain(|t| t.estilo & MATEMATICA != 0);
                    } else {
                        p.trechos.clear();
                    }
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
        } else {
            self.campo_guardado(&instr);
        }
    }

    /// O campo que o FeatherDocs não calcula: o `HYPERLINK` vira link; os outros guardam a instrução e o
    /// resultado (que fica no texto até o fim do campo, e então vira o objeto).
    fn campo_guardado(&mut self, instr: &str) {
        let palavras: Vec<&str> = instr.split_whitespace().collect();
        match palavras.first().map(|p| p.to_ascii_uppercase()).as_deref() {
            None => {}
            Some("HYPERLINK") => {
                let alvo = instr.split('"').nth(1).unwrap_or(palavras.get(1).copied().unwrap_or("")).to_string();
                self.link = Some(if palavras.get(1) == Some(&"\\l") { format!("#{alvo}") } else { alvo });
                self.campo.link = true;
            }
            Some(_) => {
                // O objeto do campo entra já no começo do resultado (o texto dele não emenda no de antes).
                let letra = (self.corrida.estilo, self.corrida.fmt);
                if let Some(p) = &mut self.par {
                    p.trechos.push(Trecho { estilo: letra.0, fmt: letra.1, ..Trecho::objeto(featherdocs_core::campo::do_word(instr, "")) });
                    self.campo.guardado = Some((instr.to_string(), p.trechos.len()));
                }
            }
        }
    }

    /// O fim do campo guardado: o resultado sai do texto e vai para o objeto.
    fn fim_do_campo_guardado(&mut self) {
        if std::mem::take(&mut self.campo.link) {
            self.link = None;
        }
        let Some((instr, inicio)) = self.campo.guardado.take() else { return };
        let Some(p) = &mut self.par else { return };
        let resultado: Vec<Trecho> = p.trechos.drain(inicio.min(p.trechos.len())..).collect();
        let texto: String = resultado.iter().map(|t| t.texto.as_str()).collect();
        // A letra do resultado fica no objeto (a do primeiro pedaço).
        // shortcut: o resultado com mais de uma letra (negrito no meio) volta com a do primeiro pedaço.
        if let (Some(primeiro), Some(objeto)) = (resultado.first(), p.trechos.last_mut()) {
            (objeto.estilo, objeto.fmt, objeto.rev) = (primeiro.estilo, primeiro.fmt, primeiro.rev.clone());
        }
        if let Some(objeto) = p.trechos.last_mut() {
            objeto.marca = Some(Box::new(featherdocs_core::campo::do_word(&instr, &texto)));
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
                } else {
                    self.campo_guardado(&instr);
                }
            }
            Some("end") => {
                self.fim_do_campo_guardado();
                self.campo.engolir = false;
                // Campo sem resultado (o `XE` do índice): a instrução só termina aqui.
                let instr = std::mem::take(&mut self.campo.instr);
                if !instr.trim().is_empty() && !self.campo_objeto(&instr) {
                    // O campo sem resultado guardado também volta (com o resultado vazio).
                    self.campo_guardado(&instr);
                    self.fim_do_campo_guardado();
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
        } else if pai == "w:sectPr" || pai == "w:pgBorders" || (pai == "w:footnotePr" || pai == "w:endnotePr") && self.dentro("w:sectPr") {
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
            "w:hyperlink" => self.link = atributo(e, "r:id").and_then(|id| self.relacoes.get(&id).cloned()).or_else(|| atributo(e, "w:anchor").map(|a| format!("#{a}"))),
            "a:blip" | "v:imagedata" => {
                let origem = atributo(e, "r:embed").or_else(|| atributo(e, "r:link")).or_else(|| atributo(e, "r:id")).and_then(|id| self.relacoes.get(&id).cloned()).unwrap_or_default();
                let (w, h) = std::mem::take(&mut self.extensao);
                if let Some(d) = &mut self.desenho {
                    (d.origem, d.largura, d.altura) = (origem, w, h);
                }
            }
            "w:drawing" | "w:pict" => self.desenho = Some(ImagemLida::default()),
            "v:shape" => {
                // O tamanho da imagem antiga vem no estilo da forma (`width:100pt;height:50pt`).
                let estilo = atributo(e, "style").unwrap_or_default();
                let medida = |nome: &str| estilo.split(';').find_map(|d| d.trim().strip_prefix(nome)).and_then(|v| medida_css(v.trim_start_matches(':')));
                self.extensao = (medida("width").unwrap_or(0.0), medida("height").unwrap_or(0.0));
            }
            "wp:anchor" => {
                let dist = |n: &str| numero(e, n).unwrap_or(0.0) / 12700.0;
                if let Some(d) = &mut self.desenho {
                    d.props.flutuante = Some(featherdocs_core::modelo::Flutuante { atras: ligado_attr(e, "behindDoc"), distancias: [dist("distT"), dist("distB"), dist("distL"), dist("distR")], ..Default::default() });
                }
            }
            "wp:positionH" | "wp:positionV" => {
                self.posicao_aberta = if nome == "wp:positionH" { 1 } else { 2 };
                if let Some(p) = self.posicao_flutuante() {
                    p.relativo = atributo(e, "relativeFrom").unwrap_or_default();
                }
            }
            "wp:wrapSquare" | "wp:wrapTight" | "wp:wrapThrough" | "wp:wrapTopAndBottom" | "wp:wrapNone" => {
                if let Some(f) = self.desenho.as_mut().and_then(|d| d.props.flutuante.as_mut()) {
                    let q = &nome["wp:wrap".len()..];
                    f.quebra = q[..1].to_ascii_lowercase() + &q[1..];
                    f.lado = atributo(e, "wrapText").filter(|l| l != "bothSides").unwrap_or_default();
                }
            }
            "wp:docPr" => {
                if let Some(d) = &mut self.desenho {
                    d.props.descricao = atributo(e, "descr").unwrap_or_default();
                    d.props.titulo = atributo(e, "title").unwrap_or_default();
                }
            }
            "a:srcRect" => {
                if let Some(d) = &mut self.desenho {
                    d.props.recorte = ["l", "t", "r", "b"].map(|k| numero(e, k).unwrap_or(0.0) / 1000.0);
                }
            }
            "a:xfrm" if self.pai() == "pic:spPr" => {
                if let Some(d) = &mut self.desenho {
                    d.props.rotacao = numero(e, "rot").unwrap_or(0.0) / 60000.0;
                    d.props.espelho_h = ligado_attr(e, "flipH");
                    d.props.espelho_v = ligado_attr(e, "flipV");
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
        // A seção do meio do documento: o papel e as faixas ficam com ela; o resto vale para o documento.
        if self.dentro("w:pPr") {
            if let Some(par) = &mut self.par {
                let tipo = atributo(e, "w:type");
                match (pai, nome) {
                    ("w:sectPr", "w:pgSz" | "w:pgMar") => {
                        if let Some(p) = &mut par.papel_da_secao {
                            papel_do_sectpr(p, nome, e);
                        }
                        return;
                    }
                    ("w:sectPr", "w:headerReference" | "w:footerReference") => {
                        if tipo.as_deref().is_none_or(|t| t == "default") {
                            par.faixas_da_secao[usize::from(nome == "w:footerReference")] = atributo(e, "r:id");
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
        let x = &mut self.doc.extras;
        let p = &mut self.doc.pagina;
        match (pai, nome) {
            ("w:sectPr", "w:pgSz") => papel_do_sectpr(p, nome, e),
            ("w:sectPr", "w:pgMar") => {
                papel_do_sectpr(p, nome, e);
                // 708 (1,25 cm) é o de sempre do Word e o que o FeatherDocs grava.
                let distancia = |a: &str| numero(e, a).filter(|v| *v != 708.0).map_or(0.0, |v| v / 20.0);
                x.mais.distancia_cabecalho = distancia("w:header");
                x.mais.distancia_rodape = distancia("w:footer");
                x.mais.medianiz = twips_para_pt(e, "w:gutter").unwrap_or(0.0);
            }
            ("w:sectPr", "w:vAlign") => {
                x.vertical = match atributo(e, "w:val").as_deref() {
                    Some("center") => Vertical::Centro,
                    Some("bottom") => Vertical::Base,
                    _ => Vertical::Topo,
                }
            }
            ("w:sectPr", "w:lnNumType") => {
                x.numerar_linhas = if atributo(e, "w:restart").as_deref() == Some("newPage") { NumeracaoDeLinhas::PorPagina } else { NumeracaoDeLinhas::Continua };
                x.mais.passo_linhas = numero(e, "w:countBy").map_or(0, |n| n.max(0.0) as u32);
                if x.mais.passo_linhas == 1 {
                    x.mais.passo_linhas = 0;
                }
            }
            ("w:sectPr", "w:titlePg") => x.primeira_diferente = ligado(e),
            ("w:sectPr", "w:headerReference" | "w:footerReference") => {
                let k = match atributo(e, "w:type").as_deref() {
                    Some("first") => 1,
                    Some("even") => 2,
                    _ => 0,
                };
                let lista = if nome == "w:headerReference" { &mut self.cabecalhos } else { &mut self.rodapes };
                lista[k] = atributo(e, "r:id");
            }
            ("w:footnotePr" | "w:endnotePr", "w:numFmt") => {
                let f = match atributo(e, "w:val").as_deref() {
                    Some("lowerLetter" | "upperLetter") => 1,
                    Some("lowerRoman" | "upperRoman") => 2,
                    Some("chicago") => 3,
                    _ => 0,
                };
                if pai == "w:footnotePr" { x.formato_notas = f } else { x.formato_notas_fim = f }
            }
            ("w:footnotePr", "w:numRestart") => x.mais.notas_por_pagina = atributo(e, "w:val").as_deref() == Some("eachPage"),
            ("w:sectPr", "w:cols") => {
                x.mais.colunas = numero(e, "w:num").map_or(0, |n| n.clamp(0.0, 45.0) as u8);
                x.mais.espaco_colunas = if x.mais.colunas > 1 { twips_para_pt(e, "w:space").unwrap_or(0.0) } else { 0.0 };
                if x.mais.colunas <= 1 {
                    x.mais.colunas = 0;
                }
            }
            ("w:sectPr", "w:pgNumType") => {
                if let Some(n) = numero(e, "w:start") {
                    x.pagina_inicial = n.max(0.0) as u32;
                }
                x.mais.formato_pagina = atributo(e, "w:fmt").filter(|f| f != "decimal").unwrap_or_default();
            }
            ("w:pgBorders", "w:top") => {
                x.mais.borda_estilo = atributo(e, "w:val").filter(|v| v != "single").unwrap_or_default();
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
            "w:drawing" | "w:pict" => self.fim_do_desenho(),
            "wp:positionH" | "wp:positionV" => self.posicao_aberta = 0,
            "w:t" | "w:delText" => self.em_texto = false,
            "m:t" => self.em_texto_math = false,
            "w:ins" | "w:del" | "w:moveTo" | "w:moveFrom" if self.pai() != "w:rPr" && self.pai() != "w:pPr" => self.rev = None,
            "w:instrText" => self.campo.em_instr = false,
            "w:fldSimple" => {
                self.fim_do_campo_guardado();
                self.campo.oculto = false;
            }
            "w:hyperlink" => self.link = None,
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
                if let Some(t) = self.tabelas.last_mut() {
                    let l = std::mem::take(&mut t.linha);
                    t.linhas.push(l);
                }
            }
            "w:tbl" => self.fim_da_tabela(),
            _ => {}
        }
    }

    fn fim_da_celula(&mut self) {
        if let Some(t) = self.tabelas.last_mut() {
            let (linha, coluna) = (t.linhas.len(), t.linha.len());
            let mut props = std::mem::take(&mut t.prop_celula);
            // A célula junta os blocos dela no lugar dos de fora; um parágrafo só de texto fica só texto.
            let blocos = std::mem::replace(&mut self.doc.blocos, std::mem::take(&mut t.fora));
            let c = featherdocs_core::modelo::texto_dos_blocos(&blocos);
            if blocos.len() > 1 || blocos.iter().any(|b| !matches!(b.conteudo, Conteudo::Paragrafo { .. })) {
                props.blocos = blocos;
            }
            t.linha.push(c);
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
        {
            if let Some(mut t) = self.tabelas.pop() {
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
        if let Some(t) = self.tabelas.last_mut() {
            if !t.par_na_celula {
                t.par_na_celula = true;
                if p.atrib.alinhamento != Alinhamento::Esquerda {
                    t.prop_celula.alinhamento = p.atrib.alinhamento;
                }
            }
        }
        self.emitir(p);
    }

    fn acrescentar(&mut self, c: Conteudo, atrib: Atrib) {
        self.doc.acrescentar(c);
        if let Some(b) = self.doc.blocos.last_mut() {
            b.atrib = atrib;
        }
    }

    /// A posição horizontal ou vertical aberta da imagem flutuante.
    fn posicao_flutuante(&mut self) -> Option<&mut featherdocs_core::modelo::PosicaoFlutuante> {
        let f = self.desenho.as_mut()?.props.flutuante.as_mut()?;
        match self.posicao_aberta {
            1 => Some(&mut f.horizontal),
            2 => Some(&mut f.vertical),
            _ => None,
        }
    }

    /// O `w:drawing` (ou `w:pict`) fechou: a imagem entra no parágrafo, em linha ou flutuante.
    fn fim_do_desenho(&mut self) {
        let Some(mut d) = self.desenho.take() else { return };
        if d.origem.is_empty() {
            return;
        }
        if let Some(p) = &mut self.par {
            if d.props.flutuante.is_none() {
                d.posicao = Some(p.trechos.len());
            }
            p.imagens.push(d);
        }
    }

    fn emitir(&mut self, p: Par) {
        let antes = self.doc.blocos.len();
        self.emitir_blocos(p);
        // Só no corpo: o bloco de dentro da célula ou da caixa não é apontável pelo documento.
        if self.doc.blocos.len() > antes && self.tabelas.is_empty() && self.caixa_aberta.is_none() {
            let bloco = self.doc.blocos[antes].id;
            for nome in std::mem::take(&mut self.indicadores) {
                self.doc.marcadores.push(featherdocs_core::modelo::Marcador { nome, bloco });
            }
        }
    }

    fn emitir_blocos(&mut self, mut p: Par) {
        let estilo = p.estilo.as_ref().and_then(|id| self.estilos.get(id)).cloned();
        let nome = estilo.as_ref().map_or("", |e| e.nome.as_str()).to_string();
        if p.quebra_antes {
            self.doc.acrescentar(Conteudo::QuebraDePagina);
        }
        let vazio = p.trechos.iter().all(|t| t.texto.trim().is_empty());
        // A imagem em linha num parágrafo com texto fica no meio dele; a sozinha e a flutuante viram bloco.
        let mut imagens = Vec::new();
        for im in std::mem::take(&mut p.imagens).into_iter().rev() {
            match im.posicao {
                Some(i) if !vazio => p.trechos.insert(i.min(p.trechos.len()), Trecho::objeto(Marca::Imagem { origem: im.origem, largura: im.largura, altura: im.altura })),
                _ => imagens.push(im),
            }
        }
        imagens.reverse();
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
        } else if vazio && imagens.is_empty() && !p.quebra_antes && !p.quebra_depois {
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
        for im in imagens {
            self.doc.acrescentar(Conteudo::Imagem { origem: im.origem, legenda: Vec::new(), largura: im.largura, altura: im.altura });
            if let Some(b) = self.doc.blocos.last_mut() {
                b.imagem = im.props;
            }
        }
        if p.quebra_depois {
            self.doc.acrescentar(Conteudo::QuebraDePagina);
        }
        if p.coluna {
            self.doc.acrescentar(Conteudo::QuebraDeColuna);
        }
        if let Some(inicio) = p.secao {
            let id = self.doc.acrescentar(Conteudo::QuebraDeSecao { inicio });
            if let (Some(pagina), Some(b)) = (p.papel_da_secao, self.doc.blocos.last_mut()) {
                b.secao = Some(Box::new(Secao { pagina, ..Secao::default() }));
                self.secoes.push((id, p.faixas_da_secao));
            }
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
        self.doc.estilos = Estilos { numeracao, listas: std::mem::take(&mut self.numeracao.modelo), legendas: self.legendas.clone(), ..self.estilos.modelo.clone() };
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
