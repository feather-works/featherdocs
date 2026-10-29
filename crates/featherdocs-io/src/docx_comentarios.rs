//! Os comentários no `.docx`: `word/comments.xml` (um `w:comment` por comentário e por resposta)
//! e `word/commentsExtended.xml` (quem responde a quem, e se está resolvido), do jeito do Word 2013+.
//! As datas vão em ISO 8601, como o Word quer; o FeatherDocs guarda "dd/mm/aaaa".

use std::collections::HashMap;
use std::fmt::Write as _;

use featherdocs_core::modelo::{Comentario, Resposta, Trecho, ITALICO, NEGRITO, SUBLINHADO};
use quick_xml::events::Event;
use quick_xml::Reader;

const NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\"";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// "06/10/2026" → "2026-10-06T00:00:00Z" (vazio se não é uma data assim).
pub(crate) fn data_iso(d: &str) -> String {
    let p: Vec<&str> = d.trim().split('/').collect();
    match p.as_slice() {
        [dia, mes, ano] if p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit())) && ano.len() == 4 => format!("{ano}-{mes:0>2}-{dia:0>2}T00:00:00Z"),
        _ => String::new(),
    }
}

/// "2026-10-06T…" → "06/10/2026".
pub(crate) fn data_de_iso(d: &str) -> String {
    let dia = d.get(..10).unwrap_or("");
    let p: Vec<&str> = dia.split('-').collect();
    match p.as_slice() {
        [ano, mes, dia] => format!("{dia}/{mes}/{ano}"),
        _ => String::new(),
    }
}

/// O `w:id` de cada resposta: o do comentário vezes mil mais a ordem (o Word só quer que sejam únicos).
pub(crate) fn id_da_resposta(comentario: u64, k: usize) -> u64 {
    comentario * 1000 + k as u64 + 1
}

/// Os `w:id` do comentário e das respostas (as marcas no texto levam todos).
pub(crate) fn ids(c: &Comentario) -> Vec<u64> {
    std::iter::once(c.id).chain((0..c.respostas.len()).map(|k| id_da_resposta(c.id, k))).collect()
}

fn iniciais_do_nome(autor: &str) -> String {
    autor.split_whitespace().filter_map(|p| p.chars().next()).collect()
}

/// `iniciais` vazias = as do nome.
/// `trechos` = o texto com negrito, itálico e sublinhado (o `\n` abre outro parágrafo).
fn um(s: &mut String, id: u64, autor: &str, iniciais: &str, quando: &str, texto: &str, trechos: Option<&[Trecho]>, para: u64) {
    let data = data_iso(quando);
    let data = if data.is_empty() { String::new() } else { format!(" w:date=\"{data}\"") };
    let iniciais = if iniciais.is_empty() { iniciais_do_nome(autor) } else { iniciais.to_string() };
    let corpo = match trechos {
        None => format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(texto)),
        Some(trechos) => {
            let mut c = String::new();
            for t in trechos {
                let mut rpr = String::new();
                for (bit, x) in [(NEGRITO, "<w:b/>"), (ITALICO, "<w:i/>"), (SUBLINHADO, "<w:u w:val=\"single\"/>")] {
                    if t.estilo & bit != 0 {
                        rpr.push_str(x);
                    }
                }
                let rpr = if rpr.is_empty() { rpr } else { format!("<w:rPr>{rpr}</w:rPr>") };
                for (k, pedaco) in t.texto.split('\n').enumerate() {
                    if k > 0 {
                        c.push_str("</w:p><w:p>");
                    }
                    if !pedaco.is_empty() {
                        let _ = write!(c, "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(pedaco));
                    }
                }
            }
            c
        }
    };
    let _ = write!(s, "<w:comment w:id=\"{id}\" w:author=\"{}\"{data} w:initials=\"{}\"><w:p w14:paraId=\"{para:08X}\">{corpo}</w:p></w:comment>", esc(autor), esc(&iniciais));
}

/// `comments.xml` e `commentsExtended.xml`.
pub(crate) fn gravar(comentarios: &[Comentario]) -> (String, String) {
    let mut c = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:comments {NS}>");
    let mut x = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w15:commentsEx {NS}>");
    for com in comentarios {
        um(&mut c, com.id, &com.autor, &com.iniciais, &com.quando, &com.texto, com.trechos_validos(), com.id);
        let feito = if com.resolvido { "1" } else { "0" };
        let _ = write!(x, "<w15:commentEx w15:paraId=\"{:08X}\" w15:done=\"{feito}\"/>", com.id);
        for (k, r) in com.respostas.iter().enumerate() {
            let id = id_da_resposta(com.id, k);
            um(&mut c, id, &r.autor, "", &r.quando, &r.texto, None, id);
            let _ = write!(x, "<w15:commentEx w15:paraId=\"{id:08X}\" w15:paraIdParent=\"{:08X}\" w15:done=\"{feito}\"/>", com.id);
        }
    }
    c.push_str("</w:comments>");
    x.push_str("</w15:commentsEx>");
    (c, x)
}

/// Um comentário lido de `comments.xml`, antes de ligar as respostas.
#[derive(Default)]
struct Lido {
    id: u64,
    autor: String,
    iniciais: String,
    quando: String,
    texto: String,
    trechos: Vec<Trecho>,
    para: String,
}

impl Lido {
    fn empurrar(&mut self, v: &str, estilo: u8) {
        self.texto.push_str(v);
        match self.trechos.last_mut() {
            Some(t) if t.estilo == estilo => t.texto.push_str(v),
            _ => self.trechos.push(Trecho { estilo, ..Trecho::simples(v) }),
        }
    }
}

fn atributo(e: &quick_xml::events::BytesStart, nome: &str) -> String {
    crate::docx::atributo(e, nome).unwrap_or_default()
}

/// `w:b`, `w:i`: ligado sem `w:val` ou com um valor verdadeiro.
fn ligado(e: &quick_xml::events::BytesStart) -> bool {
    !matches!(atributo(e, "w:val").as_str(), "0" | "false" | "off")
}

fn ler_comments(xml: &[u8]) -> Vec<Lido> {
    let mut r = Reader::from_reader(xml);
    let (mut v, mut atual, mut em_t) = (Vec::new(), None::<Lido>, false);
    // A letra da corrida aberta: negrito, itálico, sublinhado.
    let mut estilo = 0u8;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                "w:comment" => atual = Some(Lido { id: atributo(&e, "w:id").parse().unwrap_or(0), autor: atributo(&e, "w:author"), iniciais: atributo(&e, "w:initials"), quando: data_de_iso(&atributo(&e, "w:date")), ..Lido::default() }),
                "w:p" => {
                    if let Some(c) = &mut atual {
                        if c.para.is_empty() {
                            c.para = atributo(&e, "w14:paraId");
                        } else {
                            c.empurrar("\n", 0);
                        }
                    }
                }
                "w:t" => em_t = true,
                "w:r" => estilo = 0,
                "w:b" if ligado(&e) => estilo |= NEGRITO,
                "w:i" if ligado(&e) => estilo |= ITALICO,
                "w:u" if !matches!(atributo(&e, "w:val").as_str(), "none" | "0") => estilo |= SUBLINHADO,
                _ => {}
            },
            Ok(Event::Text(t)) if em_t => {
                if let Some(c) = &mut atual {
                    c.empurrar(&t.xml10_content(), estilo);
                }
            }
            Ok(Event::GeneralRef(g)) if em_t => {
                if let Some(c) = &mut atual {
                    c.empurrar(&crate::docx::referencia(&g), estilo);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "w:t" => em_t = false,
                "w:comment" => v.extend(atual.take().map(|mut c| {
                    // Sem formatação nenhuma, o texto basta.
                    if c.trechos.iter().all(|t| t.estilo == 0) {
                        c.trechos.clear();
                    }
                    c
                })),
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    v
}

/// `paraId` → (resolvido, `paraId` do pai).
fn ler_extended(xml: &[u8]) -> HashMap<String, (bool, String)> {
    let mut r = Reader::from_reader(xml);
    let mut m = HashMap::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == "w15:commentEx" => {
                let feito = matches!(atributo(&e, "w15:done").as_str(), "1" | "true");
                m.insert(atributo(&e, "w15:paraId"), (feito, atributo(&e, "w15:paraIdParent")));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    m
}

/// Os comentários (com as respostas e o "resolvido") e o conjunto dos `w:id` que são respostas
/// (as marcas deles no texto não viram comentário à parte).
pub(crate) fn ler(comments: &[u8], extended: Option<&[u8]>) -> (Vec<Comentario>, Vec<u64>) {
    let lidos = ler_comments(comments);
    let ext = extended.map(ler_extended).unwrap_or_default();
    let mut principais: Vec<Comentario> = Vec::new();
    let mut pai_de: Vec<(String, Resposta, u64)> = Vec::new();
    let mut para_de: HashMap<String, u64> = HashMap::new();
    for l in lidos {
        let (feito, pai) = ext.get(&l.para).cloned().unwrap_or_default();
        if pai.is_empty() {
            para_de.insert(l.para.clone(), l.id);
            // As iniciais que são as do nome ficam vazias (o gravador as refaz).
            let iniciais = if l.iniciais == iniciais_do_nome(&l.autor) { String::new() } else { l.iniciais };
            principais.push(Comentario { trechos: l.trechos, iniciais, id: l.id, autor: l.autor, quando: l.quando, texto: l.texto, resolvido: feito, respostas: Vec::new() });
        } else {
            pai_de.push((pai, Resposta { autor: l.autor, quando: l.quando, texto: l.texto }, l.id));
        }
    }
    let mut respostas = Vec::new();
    for (pai, r, id) in pai_de {
        if let Some(c) = para_de.get(&pai).and_then(|pid| principais.iter_mut().find(|c| c.id == *pid)) {
            c.respostas.push(r);
            respostas.push(id);
        }
    }
    (principais, respostas)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn comentarios_com_respostas_e_resolvido_vao_e_voltam() {
        let c = vec![
            Comentario { trechos: Vec::new(), iniciais: String::new(), id: 1, autor: "Ana Lima".into(), quando: "06/10/2026".into(), texto: "Veja <isto> & aquilo".into(), resolvido: true, respostas: vec![Resposta { autor: "Bia".into(), quando: "07/10/2026".into(), texto: "Visto".into() }] },
            Comentario { trechos: Vec::new(), iniciais: String::new(), id: 2, autor: "Bia".into(), quando: String::new(), texto: "Outro".into(), resolvido: false, respostas: Vec::new() },
        ];
        let (xml, ext) = gravar(&c);
        let (lidos, respostas) = ler(xml.as_bytes(), Some(ext.as_bytes()));
        assert_eq!(lidos, c);
        assert_eq!(respostas, vec![id_da_resposta(1, 0)]);
        assert_eq!(data_iso("6/10/2026"), "2026-10-06T00:00:00Z");
        assert_eq!(data_de_iso("2026-10-06T12:00:00Z"), "06/10/2026");
    }
}
