//! `.docx` (Word, Google Docs, LibreOffice) para a árvore de blocos. O `.docx` é
//! um ZIP de XMLs; aqui só se lê o que vira conteúdo: `word/document.xml` (em
//! fluxo, sem montar a árvore DOM), `styles.xml` (o que é título, os estilos com
//! nome), `numbering.xml` (o tipo de cada lista e a numeração dos títulos), as
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

use featherdocs_core::modelo::{Atrib, Celula, ConfigPagina, Conteudo, Documento, Estilo, Faixas, Fmt, InicioDeSecao, Lista, Marca, Numeracao, NumeracaoDeLinhas, Revisao, TipoDeRevisao, Trecho, Vertical, CODIGO, QUEBRA_DE_LINHA, MATEMATICA, SERIFA};
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

pub use crate::docx_gravar::gravar;
use crate::docx_gravar::{CAMPOS, MARCADORES_PADRAO, ROTULOS_LEGENDA};

type Zip<'a> = zip::ZipArchive<Cursor<&'a [u8]>>;

/// Lê os bytes de um `.docx`.
pub fn ler(bytes: &[u8]) -> Result<Documento, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Isto não é um .docx válido (o .doc antigo não é suportado): {e}"))?;
    let corpo = parte(&mut zip, "word/document.xml").ok_or("O .docx está incompleto: falta word/document.xml.")?;
    let numeracao = parte(&mut zip, "word/numbering.xml").map(|x| numeracao(&x)).unwrap_or_default();
    let estilos = parte(&mut zip, "word/styles.xml").map(|x| estilos(&x)).unwrap_or_default();
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
    }
    let faixas = [l.cabecalho.take(), l.rodape.take()].map(|rel| rel.and_then(|id| l.relacoes.get(&id).cloned()).and_then(|alvo| parte(&mut zip, &caminho_em_word(&alvo))).map(|x| ler_faixa(&x)));
    let mut doc = l.doc;
    doc.pagina = papel_conhecido(doc.pagina);
    let [cabecalho, rodape] = faixas;
    doc.extras.cabecalho = cabecalho.unwrap_or_default();
    doc.extras.rodape = rodape.unwrap_or_default();
    unir_codigo(&mut doc);
    embutir_imagens(&mut doc, &mut zip);
    sacola::embutir(&mut doc, &mut zip);
    if let Some(x) = parte(&mut zip, "docProps/core.xml") {
        propriedades(&x, &mut doc);
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
fn ler_notas(xml: &[u8], estilos: HashMap<String, EstiloDocx>, fim: bool) -> HashMap<(bool, String), Vec<Trecho>> {
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

/// Uma propriedade de caractere (`w:rPr`). `false` se não é uma delas. A fonte fica fora do mapa
/// (`docx/mapa.rs`): o nome vira um bit por semelhança.
fn prop_caractere(nome: &str, e: &BytesStart, estilo: &mut u8, fmt: &mut Fmt) -> bool {
    if nome == "w:rFonts" {
        let f = atributo(e, "w:ascii").or_else(|| atributo(e, "w:hAnsi")).unwrap_or_default().to_lowercase();
        if ["courier", "consolas", "mono", "menlo"].iter().any(|m| f.contains(m)) {
            *estilo |= CODIGO;
        } else if ["times", "serif", "georgia", "cambria", "garamond", "book antiqua", "palatino"].iter().any(|m| f.contains(m)) && !f.contains("sans") {
            *estilo |= SERIFA;
        }
        return true;
    }
    let mut c = (*estilo, *fmt);
    let achou = mapa::ler(mapa::CARACTERE, nome, e, &mut c);
    (*estilo, *fmt) = c;
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
}

// ---------------------------------------------------------------------------
// Estilos
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct EstiloDocx {
    /// O nome em minúsculas (para reconhecer "heading 1", "quote"…).
    nome: String,
    nome_original: String,
    nivel: Option<u8>,
    base: Option<String>,
    numerado: bool,
    bits: u8,
    fmt: Fmt,
    atrib: Atrib,
}

/// `styleId` → estilo (nível de título pelo nome, por `outlineLvl` ou pelo estilo de base).
fn estilos(xml: &[u8]) -> HashMap<String, EstiloDocx> {
    let mut saida = HashMap::new();
    let mut leitor = Reader::from_reader(xml);
    let mut atual: Option<(String, EstiloDocx)> = None;
    let mut em_ppr = false;
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let nome = nome_de(e);
                if nome == "w:style" {
                    atual = atributo(e, "w:styleId").map(|id| (id, EstiloDocx::default()));
                } else if nome == "w:pPr" {
                    em_ppr = matches!(ev, Event::Start(_));
                } else if let Some((_, s)) = &mut atual {
                    propriedade_de_estilo(&nome, e, s, em_ppr);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "w:style" => {
                    if let Some((id, s)) = atual.take() {
                        saida.insert(id, s);
                    }
                }
                "w:pPr" => em_ppr = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    resolver_niveis(&mut saida);
    saida
}

fn propriedade_de_estilo(nome: &str, e: &BytesStart, s: &mut EstiloDocx, em_ppr: bool) {
    match nome {
        "w:name" => {
            s.nome_original = atributo(e, "w:val").unwrap_or_default();
            s.nome = s.nome_original.to_lowercase();
            s.nivel = s.nivel.or(nivel_do_nome(&s.nome));
        }
        "w:basedOn" => s.base = atributo(e, "w:val"),
        "w:outlineLvl" => {
            if let Some(v) = atributo(e, "w:val").and_then(|v| v.parse::<u8>().ok()).filter(|v| *v < 9) {
                s.nivel.get_or_insert((v + 1).min(6));
            }
        }
        "w:numId" => s.numerado = atributo(e, "w:val").is_some_and(|v| v != "0"),
        _ if em_ppr => prop_paragrafo(nome, e, &mut s.atrib),
        _ => {
            prop_caractere(nome, e, &mut s.bits, &mut s.fmt);
        }
    }
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
    /// `(numId, ilvl)` → tipo.
    listas: HashMap<(String, u8), TipoDeLista>,
    /// A numeração dos títulos (um nível ligado a um estilo por `w:pStyle`): "Capítulo %1" ou "%1".
    titulos: Option<Numeracao>,
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
    fmt: String,
    texto: String,
    estilo: Option<String>,
}

fn nivel<'a>(abstratas: &'a mut HashMap<String, HashMap<u8, NivelLido>>, abs_id: &str, ilvl: u8) -> &'a mut NivelLido {
    abstratas.entry(abs_id.to_string()).or_default().entry(ilvl).or_default()
}

fn numeracao(xml: &[u8]) -> Numeracoes {
    let mut abstratas: HashMap<String, HashMap<u8, NivelLido>> = HashMap::new();
    let mut nums: HashMap<String, String> = HashMap::new();
    let mut leitor = Reader::from_reader(xml);
    let (mut abs_id, mut num_id, mut ilvl) = (String::new(), String::new(), 0u8);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let nome = nome_de(e);
                match nome.as_str() {
                    "w:abstractNum" => {
                        abs_id = atributo(e, "w:abstractNumId").unwrap_or_default();
                        num_id.clear();
                    }
                    "w:lvl" if num_id.is_empty() => ilvl = atributo(e, "w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0),
                    "w:numFmt" if num_id.is_empty() => nivel(&mut abstratas, &abs_id, ilvl).fmt = atributo(e, "w:val").unwrap_or_default(),
                    "w:lvlText" if num_id.is_empty() => nivel(&mut abstratas, &abs_id, ilvl).texto = atributo(e, "w:val").unwrap_or_default(),
                    "w:pStyle" if num_id.is_empty() => nivel(&mut abstratas, &abs_id, ilvl).estilo = atributo(e, "w:val"),
                    "w:num" => num_id = atributo(e, "w:numId").unwrap_or_default(),
                    "w:abstractNumId" if !num_id.is_empty() => {
                        nums.insert(num_id.clone(), atributo(e, "w:val").unwrap_or_default());
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut saida = Numeracoes::default();
    for (num, abs) in nums {
        let Some(niveis) = abstratas.get(&abs) else { continue };
        // O marcador do nível 0 diz o estilo da lista toda ("•" = o padrão, que varia por nível).
        let primeiro = niveis.get(&0).map_or("", |n| n.texto.as_str());
        for (lvl, n) in niveis {
            let texto = if n.fmt == "bullet" && primeiro != MARCADORES_PADRAO[0] { primeiro } else { n.texto.as_str() };
            saida.listas.insert((num.clone(), *lvl), tipo_do_nivel(&n.fmt, texto, if texto == primeiro { 0 } else { *lvl }));
        }
    }
    let primeiro_de_titulo = abstratas.values().filter_map(|niveis| niveis.get(&0)).find(|n| n.estilo.is_some());
    saida.titulos = primeiro_de_titulo.map(|n| if n.texto.chars().any(char::is_alphabetic) { Numeracao::Capitulo } else { Numeracao::Decimal });
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

fn propriedades(xml: &[u8], doc: &mut Documento) {
    let mut leitor = Reader::from_reader(xml);
    let mut em = String::new();
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(e) => em = nome_de(&e),
            Event::Text(t) => {
                let v = t.xml10_content().into_owned();
                match em.as_str() {
                    "dc:title" => doc.meta.titulo.push_str(&v),
                    "dc:creator" => doc.meta.autor.push_str(&v),
                    _ => {}
                }
            }
            Event::GeneralRef(r) => match em.as_str() {
                "dc:title" => doc.meta.titulo.push_str(&referencia(&r)),
                "dc:creator" => doc.meta.autor.push_str(&referencia(&r)),
                _ => {}
            },
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

/// Os pedaços de um cabeçalho ou rodapé (separados por tabulação) e o alinhamento.
fn ler_faixa(xml: &[u8]) -> Faixas {
    let mut leitor = Reader::from_reader(xml);
    let mut pedacos = vec![String::new()];
    let (mut em_t, mut em_instr, mut oculto, mut em_tabs) = (false, false, false, false);
    let mut instr = String::new();
    let mut jc = String::new();
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let vazio = matches!(ev, Event::Empty(_));
                match nome_de(e).as_str() {
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
                    juntar(&mut pedacos, &v);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    faixas_de(pedacos.into_iter().map(|p| p.trim().to_string()).collect(), &jc)
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
    outline: Option<u8>,
    quebra_antes: bool,
    quebra_depois: bool,
    /// Quebra de coluna no parágrafo (`w:br w:type="column"`).
    coluna: bool,
    /// O parágrafo fecha uma seção (`w:sectPr` no `w:pPr`): onde a seguinte começa.
    secao: Option<InicioDeSecao>,
    trechos: Vec<Trecho>,
    imagens: Vec<(String, f32, f32)>,
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

#[derive(Default, Clone, Copy)]
struct Corrida {
    estilo: u8,
    fmt: Fmt,
}

#[derive(Default)]
struct Tabela {
    linhas: Vec<Vec<Celula>>,
    linha: Vec<Celula>,
    celula: Celula,
    cabecalho: bool,
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
    estilos: HashMap<String, EstiloDocx>,
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
    campo: Campo,
    /// Logo depois do número da legenda: o separador (" – ") some.
    apos_numero: bool,
    cabecalho: Option<String>,
    rodape: Option<String>,
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
    fn novo(estilos: HashMap<String, EstiloDocx>, numeracao: Numeracoes, relacoes: HashMap<String, String>) -> Self {
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
            campo: Campo::default(),
            apos_numero: false,
            cabecalho: None,
            rodape: None,
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
        match nome.as_str() {
            "w:p" => {
                let trechos = self.controles_pendentes.drain(..).map(|xml| Trecho::objeto(Marca::Controle { xml, fim: false, bloco: true })).collect();
                self.par = Some(Par { engolido: self.campo.engolir, trechos, ..Par::default() });
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
                if let Some(t) = &mut self.tabela {
                    t.celula.clear();
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
            prop_caractere(nome, e, &mut self.corrida.estilo, &mut self.corrida.fmt);
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
        } else if pai == "w:sectPr" || pai == "w:pgBorders" {
            self.secao(nome, e, &pai);
            return;
        }
        self.solto(nome, e);
    }

    fn solto(&mut self, nome: &str, e: &BytesStart) {
        match nome {
            "w:pStyle" => self.com_par(|p| p.estilo = atributo(e, "w:val")),
            "w:numId" => self.com_par(|p| p.num_id = atributo(e, "w:val")),
            "w:ilvl" => self.com_par(|p| p.ilvl = atributo(e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(0)),
            "w:outlineLvl" => self.com_par(|p| p.outline = atributo(e, "w:val").and_then(|v| v.parse::<u8>().ok()).filter(|v| *v < 9).map(|v| (v + 1).min(6))),
            "w:pageBreakBefore" => self.com_par(|p| p.quebra_antes = true),
            "w:tab" if self.pai() == "w:r" && self.par.is_some() => self.empurrar("\t"),
            "w:br" => self.quebra_de_linha(e),
            "w:hyperlink" => self.link = atributo(e, "r:id").and_then(|id| self.relacoes.get(&id).cloned()).or_else(|| atributo(e, "w:anchor").map(|a| format!("#{a}"))),
            "a:blip" => {
                let origem = atributo(e, "r:embed").or_else(|| atributo(e, "r:link")).and_then(|id| self.relacoes.get(&id).cloned()).unwrap_or_default();
                let (w, h) = std::mem::take(&mut self.extensao);
                self.com_par(|p| p.imagens.push((origem, w, h)));
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
            let c = std::mem::take(&mut t.celula);
            t.linha.push(c);
        }
    }

    fn fim_da_tabela(&mut self) {
        self.prof_tabela = self.prof_tabela.saturating_sub(1);
        if self.prof_tabela == 0 {
            if let Some(t) = self.tabela.take() {
                if !t.linhas.is_empty() {
                    self.doc.acrescentar(Conteudo::Tabela { cabecalho: t.cabecalho, linhas: t.linhas });
                }
            }
        }
    }

    fn fim_do_paragrafo(&mut self) {
        self.apos_numero = false;
        let Some(p) = self.par.take() else { return };
        if let Some(t) = &mut self.tabela {
            if !t.celula.is_empty() && !p.trechos.is_empty() {
                t.celula.push(Trecho::simples(" "));
            }
            t.celula.extend(p.trechos);
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
                self.acrescentar(Conteudo::Paragrafo { texto: p.trechos.clone() }, p.atrib);
            }
        } else if !vazio {
            let (c, atrib) = self.bloco_de_texto(&p, estilo.as_ref(), p.estilo.as_deref());
            self.acrescentar(c, atrib);
        }
        for (origem, largura, altura) in p.imagens {
            self.doc.acrescentar(Conteudo::Imagem { origem, legenda: Vec::new(), largura, altura });
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

    fn bloco_de_texto(&self, p: &Par, estilo: Option<&EstiloDocx>, id: Option<&str>) -> (Conteudo, Atrib) {
        let texto = p.trechos.clone();
        let nome = estilo.map_or("", |e| e.nome.as_str());
        let mut atrib = p.atrib;
        atrib.estilo = id.and_then(|i| i.strip_prefix("FDEstilo")).and_then(|n| n.parse().ok()).unwrap_or(0);
        if let Some(nivel) = estilo.and_then(|e| e.nivel).or(p.outline) {
            atrib.sem_numero = p.num_id.as_deref() == Some("0");
            return (Conteudo::Titulo { nivel, texto }, atrib);
        }
        if let Some(num) = p.num_id.as_ref().filter(|n| n.as_str() != "0") {
            let tipo = self.numeracao.listas.get(&(num.clone(), p.ilvl)).copied().unwrap_or_default();
            atrib.lista = tipo.lista;
            atrib.marcado = tipo.marcado;
            // O `w:ind` do item conta da margem; no FeatherDocs o recuo é o que passa do nível.
            atrib.recuo_esq = (atrib.recuo_esq - crate::docx_gravar::RECUO_DO_NIVEL * f32::from(p.ilvl + 1)).max(0.0);
            return (Conteudo::Item { ordenado: tipo.numerada, nivel: p.ilvl.min(5), texto }, atrib);
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

    /// Os estilos com nome do FeatherDocs (`FDEstilo<id>`) e a numeração dos títulos voltam ao documento.
    fn estilos_do_documento(&mut self) {
        let mut lista: Vec<Estilo> = self
            .estilos
            .iter()
            .filter_map(|(id, s)| {
                let n: u32 = id.strip_prefix("FDEstilo")?.parse().ok()?;
                let a = &s.atrib;
                Some(Estilo { id: n, nome: s.nome_original.clone(), base: s.nivel.unwrap_or(0), bits: s.bits, tamanho: s.fmt.tamanho, cor: s.fmt.cor, alinhamento: a.alinhamento, antes: a.antes, depois: a.depois, entrelinha: a.entrelinha })
            })
            .collect();
        lista.sort_by_key(|e| e.id);
        self.doc.estilos.lista = lista;
        let titulos_numerados = self.estilos.values().any(|s| s.numerado && nivel_do_nome(&s.nome).is_some());
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
