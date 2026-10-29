//! A sacola de interoperabilidade do `.docx`: o que o modelo ainda não entende (forma, gráfico,
//! SmartArt, objeto OLE, o `w:sdtPr` dos controles de conteúdo) é guardado como o XML original, com
//! as declarações de espaço de nomes da raiz e as partes do pacote que ele aponta, e devolvido igual
//! pelo gravador. É o `InteropGrabBag` do LibreOffice. Ler: `Captura` e `embutir`; gravar:
//! `trocar_rids` (os `r:id` ganham números novos no pacote gravado).

use std::collections::{HashMap, HashSet};

use featherdocs_core::modelo::{Documento, Marca, ParteWord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::{atributo, caminho_em_word, parte, Zip};

/// As declarações `xmlns:…` de um elemento (a raiz da parte), como estão no arquivo.
pub(super) fn declaracoes(e: &BytesStart) -> String {
    e.attributes().flatten().filter(|a| a.key.as_ref().starts_with("xmlns")).map(|a| format!(" {}=\"{}\"", a.key.as_ref(), a.value)).collect()
}

/// Um elemento sendo guardado: o XML remontado dos eventos e o que se viu dentro dele.
pub(super) struct Captura {
    xml: String,
    /// As declarações da raiz da parte e onde elas entram no XML (logo depois do nome do elemento).
    ns: String,
    onde: usize,
    /// A profundidade da pilha de elementos onde ele começou.
    pub(super) prof: usize,
    pub(super) raiz: String,
    /// O `uri` do `a:graphicData` (o que o desenho é).
    uri: String,
    /// Tem caixa de texto (`wps:txbx`) ou imagem (`a:blip`): o leitor de sempre cuida dele.
    caixa: bool,
    imagem: bool,
    /// O programa do objeto OLE (`Excel.Sheet.12`).
    programa: String,
}

impl Captura {
    /// Começa no elemento `e`; `ns` são as declarações da raiz da parte.
    pub(super) fn nova(e: &BytesStart, vazio: bool, ns: &str, prof: usize) -> Self {
        let raiz = e.name().as_ref().to_string();
        let mut c = Self { xml: String::new(), ns: ns.to_string(), onde: raiz.len() + 1, prof, raiz, uri: String::new(), caixa: false, imagem: false, programa: String::new() };
        c.abre(e, vazio);
        c
    }

    /// O XML guardado, com as declarações da raiz que ele usa e ainda não tem.
    pub(super) fn xml(mut self) -> String {
        let tag = self.xml[..self.xml.find('>').unwrap_or(self.xml.len())].to_string();
        let faltam: String = self
            .ns
            .split(" xmlns")
            .filter(|d| !d.is_empty())
            .filter(|d| {
                let prefixo = d.split('=').next().unwrap_or("");
                !tag.contains(&format!("xmlns{prefixo}=")) && (prefixo.is_empty() || self.xml.contains(&format!("{}:", &prefixo[1..])))
            })
            .map(|d| format!(" xmlns{d}"))
            .collect();
        self.xml.insert_str(self.onde, &faltam);
        self.xml
    }

    pub(super) fn abre(&mut self, e: &BytesStart, vazio: bool) {
        let tag: &str = e;
        self.xml.push_str(&format!("<{tag}{}>", if vazio { "/" } else { "" }));
        self.olhar(e);
    }

    /// Texto, referência (`&amp;`) e fim de elemento, como estavam.
    pub(super) fn evento(&mut self, ev: &Event) {
        match ev {
            Event::End(e) => self.xml.push_str(&format!("</{}>", &**e)),
            Event::Text(t) => self.xml.push_str(t),
            Event::GeneralRef(r) => self.xml.push_str(&format!("&{};", &**r)),
            Event::CData(c) => self.xml.push_str(&format!("<![CDATA[{}]]>", &**c)),
            _ => {}
        }
    }

    fn olhar(&mut self, e: &BytesStart) {
        match e.name().as_ref() {
            "a:graphicData" => self.uri = atributo(e, "uri").unwrap_or_default(),
            "wps:txbx" | "v:textbox" => self.caixa = true,
            "a:blip" => self.imagem = true,
            "o:OLEObject" => self.programa = atributo(e, "ProgID").unwrap_or_default(),
            _ => {}
        }
    }

    /// O que o objeto é, para o quadro na tela; `None` = não vai para a sacola (imagem, caixa de texto).
    pub(super) fn resumo(&self) -> Option<String> {
        if self.raiz == "w:object" {
            return Some(if self.programa.is_empty() { "objeto OLE".into() } else { format!("objeto OLE ({})", self.programa) });
        }
        if self.caixa {
            return Some("caixa de texto".into());
        }
        if self.imagem || self.uri.is_empty() || self.uri.ends_with("/picture") {
            return None;
        }
        let u = self.uri.as_str();
        Some(
            if u.contains("chart") {
                "gráfico"
            } else if u.contains("diagram") {
                "SmartArt"
            } else if u.contains("wordprocessingShape") {
                "forma"
            } else if u.contains("wordprocessingGroup") {
                "grupo de formas"
            } else if u.contains("wordprocessingCanvas") {
                "tela de desenho"
            } else {
                "objeto"
            }
            .into(),
        )
    }
}

/// Onde estão os valores dos atributos `r:…` (`r:id`, `r:embed`, `r:dm`…) no XML.
fn valores_r(xml: &str) -> Vec<(usize, usize)> {
    let mut saida: Vec<(usize, usize)> = Vec::new();
    for (i, _) in xml.match_indices("r:") {
        // Um `r:` escrito dentro do valor de outro atributo não é atributo.
        if saida.last().is_some_and(|(_, fim)| i < *fim) || !xml[..i].ends_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let resto = &xml[i + 2..];
        let nome = resto.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(resto.len());
        let depois = &resto[nome..];
        let Some(aspa) = depois.strip_prefix('=').and_then(|d| d.chars().next()).filter(|c| *c == '"' || *c == '\'') else { continue };
        let inicio = i + 2 + nome + 2;
        if let Some(fim) = xml[inicio..].find(aspa) {
            saida.push((inicio, inicio + fim));
        }
    }
    saida
}

#[test]
fn r_dentro_do_valor_de_outro_atributo_nao_conta() {
    let xml = "<v:shape r:id='a r:id=\"rId5\"'/>";
    assert_eq!(valores_r(xml).len(), 1);
    let novos = HashMap::from([("a r:id=\"rId5\"".to_string(), "rId9".to_string()), ("rId5".to_string(), "rId8".to_string())]);
    assert_eq!(trocar_rids(xml, &novos), "<v:shape r:id='rId9'/>");
}

/// Troca os `r:id` do XML pelos novos (os que não estão no mapa ficam).
pub(crate) fn trocar_rids(xml: &str, novos: &HashMap<String, String>) -> String {
    let mut s = String::with_capacity(xml.len());
    let mut de = 0;
    for (a, b) in valores_r(xml) {
        if let Some(n) = novos.get(&xml[a..b]) {
            s.push_str(&xml[de..a]);
            s.push_str(n);
            de = b;
        }
    }
    s.push_str(&xml[de..]);
    s
}

/// As relações de uma parte: `Id` → `(Type, Target, externa)`.
fn relacoes(xml: &[u8]) -> HashMap<String, (String, String, bool)> {
    let mut saida = HashMap::new();
    let mut leitor = Reader::from_reader(xml);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) if e.name().as_ref() == "Relationship" => {
                if let (Some(id), Some(alvo)) = (atributo(e, "Id"), atributo(e, "Target")) {
                    let externa = atributo(e, "TargetMode").as_deref() == Some("External");
                    saida.insert(id, (atributo(e, "Type").unwrap_or_default(), alvo, externa));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    saida
}

/// O `ContentType` de cada parte (`Override`) e de cada extensão (`Default`).
fn tipos_de_conteudo(xml: &[u8]) -> (HashMap<String, String>, HashMap<String, String>) {
    let (mut partes, mut extensoes) = (HashMap::new(), HashMap::new());
    let mut leitor = Reader::from_reader(xml);
    while let Ok(ev) = leitor.read_event() {
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let tipo = atributo(e, "ContentType").unwrap_or_default();
                if let Some(p) = atributo(e, "PartName").filter(|_| e.name().as_ref() == "Override") {
                    partes.insert(p.trim_start_matches('/').to_string(), tipo);
                } else if let Some(x) = atributo(e, "Extension").filter(|_| e.name().as_ref() == "Default") {
                    extensoes.insert(x.to_ascii_lowercase(), tipo);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    (partes, extensoes)
}

/// `alvo` relativo à pasta da parte `de` (`word/charts/chart1.xml` + `../embeddings/x.xlsx`).
fn resolver(de: &str, alvo: &str) -> String {
    if let Some(absoluto) = alvo.strip_prefix('/') {
        return absoluto.to_string();
    }
    let mut pedacos: Vec<&str> = de.split('/').collect();
    pedacos.pop();
    for p in alvo.split('/') {
        match p {
            ".." => {
                pedacos.pop();
            }
            "." | "" => {}
            p => pedacos.push(p),
        }
    }
    pedacos.join("/")
}

/// Os objetos da sacola trazem as partes do pacote que o XML deles aponta (e as que essas apontam).
pub(super) fn embutir(doc: &mut Documento, zip: &mut Zip) {
    let rels = parte(zip, "word/_rels/document.xml.rels").map(|x| relacoes(&x)).unwrap_or_default();
    let (por_parte, por_extensao) = parte(zip, "[Content_Types].xml").map(|x| tipos_de_conteudo(&x)).unwrap_or_default();
    let conteudo = |caminho: &str| por_parte.get(caminho).or_else(|| caminho.rsplit('.').next().and_then(|x| por_extensao.get(&x.to_ascii_lowercase()))).cloned().unwrap_or_default();
    featherdocs_core::modelo::cada_bloco_mut(&mut doc.blocos, &mut |b| {
        for t in b.conteudo.textos_mut() {
            for tr in t.iter_mut() {
                let Some(Marca::ObjetoWord { xml, partes, .. }) = tr.marca.as_deref_mut() else { continue };
                let mut vistos = HashSet::new();
                for (a, z) in valores_r(xml) {
                    let rid = &xml[a..z];
                    let Some((tipo, alvo, externa)) = rels.get(rid) else { continue };
                    if !vistos.insert(rid.to_string()) {
                        continue;
                    }
                    let caminho = if *externa { alvo.clone() } else { caminho_em_word(alvo) };
                    let bytes = if *externa { Vec::new() } else { parte(zip, &caminho).unwrap_or_default() };
                    partes.push(ParteWord { rid: rid.to_string(), tipo: tipo.clone(), conteudo: conteudo(&caminho), caminho: caminho.clone(), externo: *externa, bytes });
                    if !*externa {
                        dependencias(zip, &caminho, partes, &conteudo);
                    }
                }
            }
        }
    });
}

/// As relações da parte `de` (`pasta/_rels/nome.rels`) e as partes que elas apontam, até o fim.
fn dependencias(zip: &mut Zip, de: &str, partes: &mut Vec<ParteWord>, conteudo: &dyn Fn(&str) -> String) {
    let (pasta, nome) = de.rsplit_once('/').unwrap_or(("", de));
    let caminho = format!("{pasta}/_rels/{nome}.rels").trim_start_matches('/').to_string();
    if partes.iter().any(|p| p.caminho == caminho) {
        return;
    }
    let Some(bytes) = parte(zip, &caminho) else { return };
    let alvos: Vec<String> = relacoes(&bytes).into_values().filter(|(_, _, externa)| !externa).map(|(_, alvo, _)| resolver(de, &alvo)).collect();
    partes.push(ParteWord { caminho, bytes, ..ParteWord::default() });
    for alvo in alvos {
        if partes.iter().any(|p| p.caminho == alvo) {
            continue;
        }
        let Some(bytes) = parte(zip, &alvo) else { continue };
        partes.push(ParteWord { conteudo: conteudo(&alvo), caminho: alvo.clone(), bytes, ..ParteWord::default() });
        dependencias(zip, &alvo, partes, conteudo);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn troca_so_os_atributos_r() {
        let xml = "<c:chart r:id=\"rId1\" nome=\"rId1\"/><dgm:relIds r:dm='rId10' r:lo=\"rId2\"/>";
        let novos = HashMap::from([("rId1".to_string(), "rIdF7".to_string()), ("rId10".to_string(), "rIdF8".to_string())]);
        assert_eq!(trocar_rids(xml, &novos), "<c:chart r:id=\"rIdF7\" nome=\"rId1\"/><dgm:relIds r:dm='rIdF8' r:lo=\"rId2\"/>");
    }

    #[test]
    fn resolve_o_caminho_relativo() {
        assert_eq!(resolver("word/charts/chart1.xml", "../embeddings/x.xlsx"), "word/embeddings/x.xlsx");
        assert_eq!(resolver("word/charts/chart1.xml", "colors1.xml"), "word/charts/colors1.xml");
        assert_eq!(resolver("word/charts/chart1.xml", "/word/media/a.png"), "word/media/a.png");
    }
}
