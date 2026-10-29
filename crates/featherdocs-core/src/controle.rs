//! O que um controle de conteúdo do Word (`w:sdt`) é, lido do `w:sdtPr` que `Marca::Controle` guarda.
//! O XML continua sendo o que vai e volta ao `.docx`; aqui só se entende o que ele diz.

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

#[cfg(test)]
mod testes {
    use super::*;

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
