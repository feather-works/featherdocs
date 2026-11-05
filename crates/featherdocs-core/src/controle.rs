//! O que um controle de conteúdo do Word (`w:sdt`) é, lido do `w:sdtPr` que `Marca::Controle` guarda.
//! O XML continua sendo o que vai e volta ao `.docx`; aqui se entende o que ele diz e se preenche o
//! campo (marcar a caixa, escolher da lista, pôr a data), mudando o texto e o `w:sdtPr` juntos.

use crate::alvo::{com_texto_ref, texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::modelo::{Marca, Trecho};
use crate::sessao::{Mudanca, Sessao};
use crate::texto::{dividir, fatia, juntar, plano};

/// O tipo do controle, pelo filho do `w:sdtPr` que o define.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TipoDeControle {
    /// Texto simples (`w:text`).
    Texto,
    /// Sem tipo dito: texto com formatação (o padrão do Word).
    #[default]
    TextoRico,
    /// Caixa de seleção (`w14:checkbox`).
    Caixa,
    /// Lista suspensa (`w:dropDownList`) ou caixa de combinação (`w:comboBox`, aceita digitar).
    Lista,
    Combinacao,
    /// Seletor de data (`w:date`).
    Data,
    /// Imagem (`w:picture`).
    Imagem,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InfoControle {
    pub tipo: TipoDeControle,
    /// O título (`w:alias`) e a marca (`w:tag`) que o autor do formulário deu.
    pub titulo: String,
    pub marca: String,
    /// Na caixa de seleção: marcada.
    pub marcado: bool,
    /// Na lista: (texto mostrado, valor) de cada item.
    pub itens: Vec<(String, String)>,
    /// No seletor de data: a data escolhida (`w:fullDate`, ISO) e o formato (`dd/MM/yyyy`).
    pub data: String,
    pub formato_data: String,
}

/// Lê o `w:sdtPr` de um controle.
pub fn info(xml: &str) -> InfoControle {
    let tags: Vec<&str> = xml.split('<').skip(1).map(|t| t.split('>').next().unwrap_or("")).filter(|t| !t.starts_with('/')).collect();
    let mut c = InfoControle::default();
    for tag in tags {
        let nome = tag.split([' ', '/']).next().unwrap_or("");
        match nome {
            "w:alias" => c.titulo = valor(tag, "w:val"),
            "w:tag" => c.marca = valor(tag, "w:val"),
            "w:text" => c.tipo = TipoDeControle::Texto,
            "w14:checkbox" => c.tipo = TipoDeControle::Caixa,
            "w14:checked" => c.marcado = matches!(valor(tag, "w14:val").as_str(), "1" | "true"),
            "w:dropDownList" => c.tipo = TipoDeControle::Lista,
            "w:comboBox" => c.tipo = TipoDeControle::Combinacao,
            "w:listItem" => c.itens.push((valor(tag, "w:displayText"), valor(tag, "w:value"))),
            "w:date" => {
                c.tipo = TipoDeControle::Data;
                c.data = valor(tag, "w:fullDate");
            }
            "w:dateFormat" => c.formato_data = valor(tag, "w:val"),
            "w:picture" => c.tipo = TipoDeControle::Imagem,
            _ => {}
        }
    }
    c
}

/// O valor do atributo `nome` na etiqueta (sem o `<`), com as entidades do XML desfeitas.
fn valor(tag: &str, nome: &str) -> String {
    let Some(resto) = tag.split(&format!(" {nome}=\"")).nth(1) else { return String::new() };
    let v = resto.split('"').next().unwrap_or("");
    v.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// A caixa marcada e a vazia, como o Word grava (`w14:checkedState`/`w14:uncheckedState` mudam isso).
const MARCADA: char = '☒';
const VAZIA: char = '☐';

impl Sessao {
    /// O controle (campo de formulário do Word) onde está o cursor: o bloco, a célula, o trecho que o
    /// abre e onde começa e acaba o texto de dentro, em caracteres. Só o controle dentro da linha; o
    /// que envolve parágrafos inteiros fica de fora.
    fn controle_do_cursor(&self) -> Option<(usize, Cel, usize, usize, usize)> {
        let foco = self.cursor.foco;
        let i = self.idx(foco.bloco)?;
        let t = texto_alvo(&self.doc.blocos[i].conteudo, foco.celula)?;
        let mut pos = 0;
        let mut aberto: Option<(usize, usize)> = None;
        for (k, tr) in t.iter().enumerate() {
            let n = tr.texto.chars().count();
            match tr.marca.as_deref() {
                Some(Marca::Controle { fim: false, bloco: false, .. }) => aberto = Some((k, pos + n)),
                Some(Marca::Controle { fim: true, .. }) => {
                    if let Some((ka, a)) = aberto.take() {
                        // O cursor logo antes da marca de abrir também conta: é onde o clique cai.
                        if (a.saturating_sub(1)..=pos + n).contains(&foco.desl) {
                            return Some((i, foco.celula, ka, a, pos));
                        }
                    }
                }
                _ => {}
            }
            pos += n;
        }
        None
    }

    /// O que é o controle do cursor e o texto que ele mostra agora.
    pub fn controle_no_cursor(&self) -> Option<(InfoControle, String)> {
        let (i, cel, ka, a, z) = self.controle_do_cursor()?;
        let t = texto_alvo(&self.doc.blocos[i].conteudo, cel)?;
        let Some(Marca::Controle { xml, .. }) = t[ka].marca.as_deref() else { return None };
        Some((info(xml), plano(&fatia(&t, a, z))))
    }

    /// O controle do cursor ainda mostra o texto de exemplo ("Clique aqui para inserir o texto").
    pub fn controle_vazio(&self) -> bool {
        self.controle_do_cursor().is_some_and(|(i, cel, ka, _, _)| {
            texto_alvo(&self.doc.blocos[i].conteudo, cel).is_some_and(|t| matches!(t[ka].marca.as_deref(), Some(Marca::Controle { xml, .. }) if xml.contains("<w:showingPlcHdr")))
        })
    }

    /// Seleciona o texto de dentro do controle do cursor (o de exemplo some ao digitar por cima).
    pub fn controle_selecionar(&mut self) -> bool {
        let Some((i, cel, _, a, z)) = self.controle_do_cursor() else { return false };
        let bloco = self.doc.blocos[i].id;
        self.cursor = Cursor { ancora: Pos { bloco, celula: cel, desl: a }, foco: Pos { bloco, celula: cel, desl: z }, x_desejado: None, pendente: None };
        true
    }

    /// Troca o texto de dentro do controle do cursor por `novo` e ajusta o `w:sdtPr` com `mudar_xml`
    /// (um Ctrl+Z). O texto de exemplo deixa de ser exemplo: sai o `w:showingPlcHdr` e o estilo cinza.
    fn trocar_controle(&mut self, novo: &str, mudar_xml: impl FnOnce(&mut String)) -> Mudanca {
        let Some((i, cel, ka, a, z)) = self.controle_do_cursor() else { return self.sem_mudanca() };
        let bloco = self.doc.blocos[i].id;
        let mut t = self.trabalho(i, i);
        com_texto_ref(&mut t.blocos[0].conteudo, cel, |tr| {
            let mut exemplo = false;
            if let Some(Marca::Controle { xml, .. }) = tr[ka].marca.as_deref_mut() {
                exemplo = xml.contains("<w:showingPlcHdr");
                *xml = xml.replace("<w:showingPlcHdr/>", "");
                mudar_xml(xml);
            }
            let mut modelo = fatia(tr, a, z).into_iter().next().unwrap_or_else(|| Trecho { marca: None, ..tr[ka].clone() });
            if exemplo {
                modelo.fmt.estilo_de_caractere = 0;
            }
            let (antes, _) = dividir(tr, a);
            let resto = fatia(tr, z, usize::MAX);
            *tr = juntar(juntar(antes, vec![Trecho { texto: novo.to_string(), marca: None, ..modelo }]), resto);
        });
        let desl = a + novo.chars().count();
        self.fechar_trabalho(t, Cursor::em(Pos { bloco, celula: cel, desl }), None)
    }

    /// Marca ou desmarca a caixa de seleção do cursor. `None` se o cursor não está numa caixa.
    pub fn controle_alternar(&mut self) -> Option<Mudanca> {
        let (c, _) = self.controle_no_cursor().filter(|(c, _)| c.tipo == TipoDeControle::Caixa)?;
        let marcar = !c.marcado;
        let novo = if marcar { MARCADA } else { VAZIA };
        Some(self.trocar_controle(&novo.to_string(), |xml| {
            let val = if marcar { "1" } else { "0" };
            if let Some(i) = xml.find("<w14:checked ") {
                let fim = xml[i..].find("/>").map_or(xml.len(), |f| i + f + 2);
                xml.replace_range(i..fim, &format!("<w14:checked w14:val=\"{val}\"/>"));
            } else if let Some(i) = xml.find("<w14:checkbox>") {
                xml.insert_str(i + "<w14:checkbox>".len(), &format!("<w14:checked w14:val=\"{val}\"/>"));
            }
        }))
    }

    /// Escolhe um item da lista (ou escreve na caixa de combinação) do controle do cursor.
    pub fn controle_escolher(&mut self, texto: &str) -> Result<Mudanca, String> {
        let (c, _) = self.controle_no_cursor().ok_or("O cursor não está num campo de formulário.")?;
        if !matches!(c.tipo, TipoDeControle::Lista | TipoDeControle::Combinacao) {
            return Err("Este campo não é uma lista.".into());
        }
        if c.tipo == TipoDeControle::Lista && !c.itens.iter().any(|(d, _)| d == texto) {
            return Err(format!("\"{texto}\" não é uma opção da lista."));
        }
        Ok(self.trocar_controle(texto, |_| {}))
    }

    /// Põe a data `dia/mês/ano` (também `aaaa-mm-dd`) no seletor de data do cursor, no formato do campo.
    pub fn controle_data(&mut self, texto: &str) -> Result<Mudanca, String> {
        let (c, _) = self.controle_no_cursor().filter(|(c, _)| c.tipo == TipoDeControle::Data).ok_or("O cursor não está num campo de data.")?;
        let (d, m, a) = ler_data(texto).ok_or_else(|| format!("Data inválida: \"{texto}\" (use dia/mês/ano)."))?;
        let mostrada = escrever_data(if c.formato_data.is_empty() { "dd/MM/yyyy" } else { &c.formato_data }, d, m, a);
        let completa = format!("{a:04}-{m:02}-{d:02}T00:00:00Z");
        Ok(self.trocar_controle(&mostrada, |xml| {
            if let Some(i) = xml.find(" w:fullDate=\"") {
                let ini = i + " w:fullDate=\"".len();
                let fim = xml[ini..].find('"').map_or(xml.len(), |f| ini + f);
                xml.replace_range(ini..fim, &completa);
            } else if let Some(i) = xml.find("<w:date") {
                xml.insert_str(i + "<w:date".len(), &format!(" w:fullDate=\"{completa}\""));
            }
        }))
    }
}

/// Dia, mês e ano de "28/10/2026", "28-10-26" ou "2026-10-28"; `None` se não é uma data de verdade.
pub fn ler_data(texto: &str) -> Option<(u32, u32, i32)> {
    let partes: Vec<&str> = texto.trim().split(['/', '-', '.']).collect();
    let [x, y, z] = partes[..] else { return None };
    let (d, m, a) = if x.len() == 4 { (z, y, x) } else { (x, y, z) };
    let (d, m, mut a): (u32, u32, i32) = (d.parse().ok()?, m.parse().ok()?, a.parse().ok()?);
    if a < 100 {
        a += 2000;
    }
    let bissexto = (a % 4 == 0 && a % 100 != 0) || a % 400 == 0;
    let dias = [31, if bissexto { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    ((1..=12).contains(&m) && d >= 1 && d <= dias[m as usize - 1]).then_some((d, m, a))
}

/// A data no formato do Word (`dd/MM/yyyy`, `d/M/yy`…); o nome do mês (`MMMM`) sai em número.
fn escrever_data(formato: &str, d: u32, m: u32, a: i32) -> String {
    // shortcut: "MMMM" e "dddd" (nomes de mês e dia) saem em número; escrever por extenso se algum formulário pedir.
    let mut s = String::new();
    let cs: Vec<char> = formato.chars().collect();
    let mut k = 0;
    while k < cs.len() {
        let c = cs[k];
        let n = cs[k..].iter().take_while(|&&x| x == c).count();
        match c {
            'd' => s.push_str(&if n >= 2 { format!("{d:02}") } else { d.to_string() }),
            'M' => s.push_str(&if n >= 2 { format!("{m:02}") } else { m.to_string() }),
            'y' => s.push_str(&if n >= 3 { format!("{a:04}") } else { format!("{:02}", a % 100) }),
            _ => s.extend(&cs[k..k + n]),
        }
        k += n;
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_controle(sdt_pr: &str, dentro: &str) -> Sessao {
        let marca = |xml: &str, fim: bool| Trecho::objeto(Marca::Controle { xml: xml.into(), fim, bloco: false });
        let texto = vec![Trecho::simples("Campo: "), marca(sdt_pr, false), Trecho::simples(dentro), marca("", true), Trecho::simples(" fim")];
        let mut d = crate::modelo::Documento::vazio();
        d.blocos = vec![crate::modelo::Bloco::novo(1, crate::modelo::Conteudo::Paragrafo { texto })];
        let mut s = Sessao::com_documento(d);
        s.cursor = Cursor::em(Pos { bloco: 1, celula: None, desl: 9 });
        s
    }

    fn xml_do_controle(s: &Sessao) -> String {
        let t = texto_alvo(&s.documento().blocos[0].conteudo, None).unwrap();
        t.iter().find_map(|x| match x.marca.as_deref() { Some(Marca::Controle { xml, fim: false, .. }) => Some(xml.clone()), _ => None }).unwrap()
    }

    #[test]
    fn caixa_marca_desmarca_e_vai_para_o_xml() {
        let mut s = com_controle("<w14:checkbox><w14:checked w14:val=\"0\"/></w14:checkbox>", "☐");
        assert_eq!(s.controle_no_cursor().map(|(c, t)| (c.tipo, t)), Some((TipoDeControle::Caixa, "☐".into())));
        s.controle_alternar().unwrap();
        assert_eq!(s.controle_no_cursor().unwrap().1, "☒");
        assert!(xml_do_controle(&s).contains("<w14:checked w14:val=\"1\"/>"));
        s.controle_alternar().unwrap();
        assert!(!s.controle_no_cursor().unwrap().0.marcado);
        s.desfazer();
        assert!(s.controle_no_cursor().unwrap().0.marcado, "cada clique é um Ctrl+Z");
    }

    #[test]
    fn lista_e_data_trocam_o_texto_e_tiram_o_exemplo() {
        let lista = "<w:showingPlcHdr/><w:dropDownList><w:listItem w:displayText=\"Sim\" w:value=\"1\"/><w:listItem w:displayText=\"Não\" w:value=\"2\"/></w:dropDownList>";
        let mut s = com_controle(lista, "Escolha um item.");
        assert!(s.controle_vazio());
        assert!(s.controle_escolher("Talvez").is_err());
        s.controle_escolher("Não").unwrap();
        assert_eq!(s.controle_no_cursor().unwrap().1, "Não");
        assert!(!s.controle_vazio(), "o texto de exemplo saiu");
        let mut s = com_controle("<w:date w:fullDate=\"2020-01-01T00:00:00Z\"><w:dateFormat w:val=\"d/M/yy\"/></w:date>", "1/1/20");
        assert!(s.controle_data("31/02/2026").is_err());
        s.controle_data("5/11/2026").unwrap();
        assert_eq!(s.controle_no_cursor().unwrap().1, "5/11/26");
        assert!(xml_do_controle(&s).contains("w:fullDate=\"2026-11-05T00:00:00Z\""));
        assert_eq!(ler_data("2026-11-05"), Some((5, 11, 2026)));
        assert_eq!(escrever_data("dd/MM/yyyy", 5, 1, 2026), "05/01/2026");
    }

    #[test]
    fn le_os_quatro_tipos_de_controle() {
        let t = info("<w:sdtPr><w:alias w:val=\"Nome &amp; sobrenome\"/><w:tag w:val=\"nome\"/><w:text/></w:sdtPr>");
        assert_eq!((t.tipo, t.titulo.as_str(), t.marca.as_str()), (TipoDeControle::Texto, "Nome & sobrenome", "nome"));
        let c = info("<w:sdtPr><w14:checkbox><w14:checked w14:val=\"1\"/></w14:checkbox></w:sdtPr>");
        assert_eq!((c.tipo, c.marcado), (TipoDeControle::Caixa, true));
        let l = info("<w:sdtPr><w:dropDownList><w:listItem w:displayText=\"Sim\" w:value=\"1\"/><w:listItem w:displayText=\"Não\" w:value=\"2\"/></w:dropDownList></w:sdtPr>");
        assert_eq!((l.tipo, l.itens), (TipoDeControle::Lista, vec![("Sim".into(), "1".into()), ("Não".into(), "2".into())]));
        let d = info("<w:sdtPr><w:date w:fullDate=\"2026-10-28T00:00:00Z\"><w:dateFormat w:val=\"dd/MM/yyyy\"/></w:date></w:sdtPr>");
        assert_eq!((d.tipo, d.data.as_str(), d.formato_data.as_str()), (TipoDeControle::Data, "2026-10-28T00:00:00Z", "dd/MM/yyyy"));
        assert_eq!(info("<w:sdtPr><w:id w:val=\"3\"/></w:sdtPr>").tipo, TipoDeControle::TextoRico);
    }
}
