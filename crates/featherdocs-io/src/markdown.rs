//! Markdown (CommonMark + tabelas, tachado, fórmulas `$…$`/`$$…$$` e cabeçalho
//! YAML) para a árvore de blocos. O `pulldown-cmark` dá eventos; aqui eles viram
//! blocos sem montar nenhuma árvore intermediária.

use featherdocs_core::inline::texto_puro;
use std::collections::HashMap;

use featherdocs_core::modelo::{Celula, Conteudo, Documento, Marca, Trecho, CODIGO, ITALICO, MATEMATICA, NEGRITO, TACHADO};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Lê um texto Markdown.
pub fn ler(texto: &str) -> Documento {
    let opcoes = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS | Options::ENABLE_TASKLISTS | Options::ENABLE_FOOTNOTES;
    let mut l = Leitor::default();
    for e in Parser::new_ext(texto, opcoes) {
        l.evento(e);
    }
    l.flush();
    l.ligar_notas();
    l.doc.acertar_ids();
    l.doc
}

#[derive(Default)]
struct Tabela {
    linhas: Vec<Vec<Celula>>,
    atual: Vec<Celula>,
    cabecalho: bool,
}

struct Leitor {
    doc: Documento,
    trechos: Vec<Trecho>,
    estilo: u8,
    links: Vec<String>,
    listas: Vec<bool>,
    item_aberto: bool,
    /// O número do primeiro item da lista aberta (`5.`), quando não é 1.
    inicio: Option<u32>,
    citacao: usize,
    codigo: Option<(String, String)>,
    tabela: Option<Tabela>,
    imagem: Option<String>,
    metadados: Option<String>,
    /// Dentro da definição de uma nota (`[^1]: …`): o rótulo dela.
    nota: Option<String>,
    /// O texto de cada nota pelo rótulo; liga-se às chamadas no fim.
    notas: HashMap<String, Vec<Trecho>>,
    /// O comentário com as fontes (`<!-- FeatherDocs: fontes … -->`) enquanto chega.
    fontes: Option<String>,
}

impl Default for Leitor {
    fn default() -> Self {
        Self {
            doc: Documento::vazio(),
            trechos: Vec::new(),
            estilo: 0,
            links: Vec::new(),
            listas: Vec::new(),
            item_aberto: false,
            inicio: None,
            citacao: 0,
            codigo: None,
            tabela: None,
            imagem: None,
            metadados: None,
            nota: None,
            notas: HashMap::new(),
            fontes: None,
        }
    }
}

fn nivel(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

impl Leitor {
    fn empurrar(&mut self, texto: &str, extra: u8) {
        if texto.is_empty() {
            return;
        }
        let estilo = self.estilo | extra;
        let link = self.links.last().cloned();
        if let Some(u) = self.trechos.last_mut() {
            if u.marca.is_none() && u.estilo == estilo && u.link == link {
                u.texto.push_str(texto);
                return;
            }
        }
        self.trechos.push(Trecho { texto: texto.to_string(), estilo, link, ..Trecho::default() });
    }

    fn acrescentar(&mut self, mut c: Conteudo) {
        for t in c.textos_mut() {
            *t = citacoes(std::mem::take(t));
        }
        self.doc.acrescentar(c);
    }

    /// Escoa o texto acumulado como o bloco que o contexto pede (item, citação ou parágrafo).
    fn flush(&mut self) {
        if self.trechos.is_empty() {
            return;
        }
        let mut texto = std::mem::take(&mut self.trechos);
        if let Some(rotulo) = &self.nota {
            let nota = self.notas.entry(rotulo.clone()).or_default();
            if !nota.is_empty() {
                nota.push(Trecho::simples(" "));
            }
            nota.append(&mut texto);
            return;
        }
        let c = if self.item_aberto && !self.listas.is_empty() {
            self.item_aberto = false;
            if let Some(v) = self.inicio.take() {
                self.acrescentar(Conteudo::Item { ordenado: true, nivel: (self.listas.len() - 1).min(5) as u8, texto });
                if let Some(b) = self.doc.blocos.last_mut() {
                    b.atrib.mais.inicio = Some(v);
                }
                return;
            }
            Conteudo::Item { ordenado: *self.listas.last().unwrap_or(&false), nivel: (self.listas.len() - 1).min(5) as u8, texto }
        } else if self.citacao > 0 {
            Conteudo::Citacao { texto }
        } else if matches!(texto_puro(&texto).trim().to_ascii_lowercase().as_str(), "[toc]" | "[sumario]" | "[sumário]") {
            Conteudo::sumario(3)
        } else if let Some(lista) = match texto_puro(&texto).trim().to_ascii_lowercase().as_str() {
            "[lof]" => Some(1),
            "[lot]" => Some(2),
            _ => None,
        } {
            Conteudo::Sumario { niveis: 1, sem_numeros: false, sem_pontilhado: false, lista, latex: false }
        } else if let Some(c) = bloco_gerado(&texto_puro(&texto)) {
            c
        } else {
            Conteudo::Paragrafo { texto }
        };
        self.acrescentar(c);
    }

    fn evento(&mut self, e: Event) {
        match e {
            Event::Start(t) => self.inicio(t),
            Event::End(t) => self.fim(t),
            Event::Text(t) => self.texto(&t),
            Event::Code(c) => self.empurrar(&c, CODIGO),
            Event::InlineMath(m) => self.empurrar(&m, MATEMATICA),
            Event::DisplayMath(m) => {
                self.flush();
                self.acrescentar(Conteudo::Equacao { latex: m.trim().to_string() });
            }
            Event::SoftBreak | Event::HardBreak => self.empurrar(" ", 0),
            Event::Rule => {
                self.flush();
                self.acrescentar(Conteudo::Regua);
            }
            Event::TaskListMarker(feito) => self.empurrar(if feito { "☑ " } else { "☐ " }, 0),
            Event::Html(h) | Event::InlineHtml(h) => self.html(&h),
            Event::FootnoteReference(r) => {
                // O rótulo fica guardado no trecho até `ligar_notas` achar a definição.
                let marca = Marca::Nota { fim: r.starts_with("fim-"), texto: Vec::new() };
                self.trechos.push(Trecho { estilo: self.estilo, rotulo: r.to_string(), ..Trecho::objeto(marca) });
            }
        }
    }

    fn texto(&mut self, t: &str) {
        if let Some(m) = &mut self.metadados {
            m.push_str(t);
        } else if let Some((_, c)) = &mut self.codigo {
            c.push_str(t);
        } else {
            self.empurrar(t, 0);
        }
    }

    fn inicio(&mut self, t: Tag) {
        match t {
            Tag::Paragraph | Tag::Heading { .. } => {}
            Tag::BlockQuote(_) => {
                self.flush();
                self.citacao += 1;
            }
            Tag::CodeBlock(k) => {
                self.flush();
                let lang = if let CodeBlockKind::Fenced(l) = k { l.split_whitespace().next().unwrap_or("").to_string() } else { String::new() };
                self.codigo = Some((lang, String::new()));
            }
            Tag::List(inicio) => {
                self.flush();
                self.listas.push(inicio.is_some());
                self.inicio = inicio.filter(|n| *n != 1).and_then(|n| u32::try_from(n).ok());
            }
            Tag::Item => {
                self.flush();
                self.item_aberto = true;
            }
            Tag::Table(_) => {
                self.flush();
                self.tabela = Some(Tabela { cabecalho: false, ..Tabela::default() });
            }
            Tag::TableHead => {
                if let Some(t) = &mut self.tabela {
                    t.cabecalho = true;
                }
            }
            Tag::Emphasis => self.estilo |= ITALICO,
            Tag::Strong => self.estilo |= NEGRITO,
            Tag::Strikethrough => self.estilo |= TACHADO,
            Tag::Link { dest_url, .. } => self.links.push(dest_url.to_string()),
            Tag::Image { dest_url, .. } => {
                self.flush();
                self.imagem = Some(dest_url.to_string());
            }
            Tag::MetadataBlock(_) => self.metadados = Some(String::new()),
            Tag::FootnoteDefinition(r) => {
                self.flush();
                self.nota = Some(r.to_string());
            }
            _ => {}
        }
    }

    /// HTML solto: só interessa o comentário com as fontes (BibTeX) que o FeatherDocs grava no fim.
    fn html(&mut self, h: &str) {
        if self.fontes.is_none() && h.trim_start().starts_with(crate::exportar::MARCA_DAS_FONTES) {
            self.fontes = Some(String::new());
        }
        let Some(f) = &mut self.fontes else { return };
        f.push_str(h);
        if !f.contains("-->") {
            return;
        }
        let corpo = f.trim_start().trim_start_matches(crate::exportar::MARCA_DAS_FONTES).split("-->").next().unwrap_or("").replace("\\-\\-", "--");
        let (primeira, bib) = corpo.split_once('\n').unwrap_or((&corpo, ""));
        if let Some(e) = primeira.trim().strip_prefix("estilo=").and_then(featherdocs_core::bibliografia::estilo_do_nome) {
            self.doc.bibliografia.estilo = e;
        }
        self.doc.bibliografia.fontes = crate::bib::ler(bib);
        self.fontes = None;
    }

    /// Põe o texto de cada nota (das definições) dentro da chamada dela.
    fn ligar_notas(&mut self) {
        for b in &mut self.doc.blocos {
            for t in b.conteudo.textos_mut() {
                for tr in t.iter_mut() {
                    if let Some(Marca::Nota { texto, .. }) = tr.marca.as_deref_mut() {
                        *texto = self.notas.get(&tr.rotulo).cloned().unwrap_or_default();
                        tr.rotulo.clear();
                    }
                }
            }
        }
    }

    fn fim(&mut self, t: TagEnd) {
        match t {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(n) => {
                let texto = std::mem::take(&mut self.trechos);
                self.acrescentar(Conteudo::Titulo { nivel: nivel(n), texto });
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.citacao = self.citacao.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some((linguagem, codigo)) = self.codigo.take() {
                    self.acrescentar(Conteudo::Codigo { linguagem, codigo: codigo.trim_end_matches('\n').to_string() });
                }
            }
            TagEnd::List(_) => {
                self.flush();
                self.listas.pop();
            }
            TagEnd::Item => {
                self.flush();
                self.item_aberto = false;
            }
            TagEnd::TableCell => {
                let c = std::mem::take(&mut self.trechos);
                if let Some(t) = &mut self.tabela {
                    t.atual.push(c);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(t) = &mut self.tabela {
                    let l = std::mem::take(&mut t.atual);
                    t.linhas.push(l);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.tabela.take() {
                    self.acrescentar(Conteudo::Tabela { cabecalho: t.cabecalho, linhas: t.linhas });
                }
            }
            TagEnd::Emphasis => self.estilo &= !ITALICO,
            TagEnd::Strong => self.estilo &= !NEGRITO,
            TagEnd::Strikethrough => self.estilo &= !TACHADO,
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::Image => {
                let legenda = std::mem::take(&mut self.trechos);
                if let Some(origem) = self.imagem.take() {
                    self.acrescentar(Conteudo::Imagem { origem, legenda, largura: 0.0, altura: 0.0 });
                }
            }
            TagEnd::FootnoteDefinition => {
                self.flush();
                self.nota = None;
            }
            TagEnd::MetadataBlock(_) => {
                if let Some(m) = self.metadados.take() {
                    self.cabecalho_yaml(&m);
                }
            }
            _ => {}
        }
    }

    /// Só `title:` e `author:`; o resto do YAML é ignorado de propósito.
    fn cabecalho_yaml(&mut self, yaml: &str) {
        for l in yaml.lines() {
            let Some((chave, valor)) = l.split_once(':') else { continue };
            let valor = valor.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
            match chave.trim() {
                "title" | "titulo" | "título" => self.doc.meta.titulo = valor,
                "author" | "autor" => self.doc.meta.autor = valor,
                _ => {}
            }
        }
    }
}

/// As marcas das listas geradas: `[REFERENCIAS]` (com `: TODAS`, também as não citadas) e `[INDICE]`.
fn bloco_gerado(texto: &str) -> Option<Conteudo> {
    let t = featherdocs_core::bibliografia::dobrar(texto.trim());
    Some(match t.as_str() {
        "[referencias]" | "[bibliografia]" => Conteudo::Bibliografia { todas: false },
        "[referencias: todas]" => Conteudo::Bibliografia { todas: true },
        "[indice]" => Conteudo::IndiceRemissivo,
        _ => return None,
    })
}

/// Uma citação do Pandoc (`[@silva2023; @lima2020, p. 4]`) → as chaves e a página.
fn ler_citacao(dentro: &str) -> Option<Marca> {
    let mut chaves = Vec::new();
    let mut pagina = String::new();
    for parte in dentro.split(';') {
        let parte = parte.trim();
        let (chave, resto) = parte.strip_prefix('@')?.split_once(',').map_or((parte.trim_start_matches('@'), ""), |(c, r)| (c, r));
        if chave.is_empty() || !chave.chars().all(|c| c.is_alphanumeric() || "_-:.".contains(c)) {
            return None;
        }
        chaves.push(chave.to_string());
        let r = resto.trim();
        if let Some(p) = r.strip_prefix("p.").or_else(|| r.strip_prefix("pp.")) {
            pagina = p.trim().to_string();
        }
    }
    Some(Marca::Citacao { chaves, pagina })
}

/// Troca as citações do Pandoc escritas no texto por objetos de citação.
fn citacoes(trechos: Vec<Trecho>) -> Vec<Trecho> {
    let mut saida = Vec::with_capacity(trechos.len());
    for t in trechos {
        if t.marca.is_some() || t.estilo & (CODIGO | MATEMATICA) != 0 || !t.texto.contains("[@") {
            saida.push(t);
            continue;
        }
        let mut resto = t.texto.as_str();
        while let Some(i) = resto.find("[@") {
            let Some(f) = resto[i..].find(']') else { break };
            let Some(m) = ler_citacao(&resto[i + 1..i + f]) else {
                saida.push(Trecho { texto: resto[..i + 2].to_string(), ..t.clone() });
                resto = &resto[i + 2..];
                continue;
            };
            if i > 0 {
                saida.push(Trecho { texto: resto[..i].to_string(), ..t.clone() });
            }
            saida.push(Trecho { estilo: t.estilo, ..Trecho::objeto(m) });
            resto = &resto[i + f + 1..];
        }
        if !resto.is_empty() {
            saida.push(Trecho { texto: resto.to_string(), ..t.clone() });
        }
    }
    featherdocs_core::texto::normalizar(&mut saida);
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use featherdocs_core::edicao::nome_do_tipo;

    fn tipos(md: &str) -> Vec<String> {
        ler(md).blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect()
    }

    #[test]
    fn lista_que_comeca_em_cinco_vai_e_volta() {
        let d = ler("5. cinco\n6. seis\n");
        assert_eq!(d.blocos.iter().map(|b| b.atrib.mais.inicio).collect::<Vec<_>>(), [Some(5), None]);
        assert_eq!(featherdocs_core::paginacao::marcadores(&d.blocos, &[]).into_iter().flatten().collect::<Vec<_>>(), ["5.", "6."]);
        assert!(crate::exportar::markdown(&d).contains("5. cinco\n1. seis"));
        assert!(crate::exportar::latex(&d).contains("\\setcounter{enumi}{4}\\item cinco"));
    }

    #[test]
    fn blocos_basicos() {
        let t = tipos("# T\n\npar **a**\n\n- x\n- y\n\n1. um\n\n> cit\n\n---\n\n```rs\nfn a(){}\n```\n");
        assert_eq!(t, vec!["titulo1", "paragrafo", "item", "item", "item_ordenado", "citacao", "regua", "codigo"]);
    }

    #[test]
    fn lista_aninhada_tem_nivel() {
        let d = ler("- a\n  - b\n    - c\n");
        let n: Vec<u8> = d.blocos.iter().map(|b| if let Conteudo::Item { nivel, .. } = b.conteudo { nivel } else { 99 }).collect();
        assert_eq!(n, vec![0, 1, 2]);
    }

    #[test]
    fn lista_solta_nao_perde_texto() {
        let d = ler("- a\n\n- b\n");
        assert_eq!(d.blocos.len(), 2);
    }

    #[test]
    fn tabela_com_cabecalho() {
        let d = ler("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let Conteudo::Tabela { cabecalho, linhas } = &d.blocos[0].conteudo else { panic!("sem tabela") };
        assert!(*cabecalho);
        assert_eq!(linhas.len(), 2);
        assert_eq!(texto_puro(&linhas[1][1]), "2");
    }

    #[test]
    fn formulas_e_sumario() {
        let d = ler("[TOC]\n\nTexto $x^2$ aqui.\n\n$$\n\\int_0^1 f\n$$\n");
        assert!(matches!(d.blocos[0].conteudo, Conteudo::Sumario { .. }));
        let Conteudo::Paragrafo { texto } = &d.blocos[1].conteudo else { panic!() };
        assert!(texto.iter().any(|t| t.estilo & MATEMATICA != 0 && t.texto == "x^2"));
        assert!(matches!(d.blocos[2].conteudo, Conteudo::Equacao { .. }));
    }

    #[test]
    fn cabecalho_yaml_vira_meta() {
        let d = ler("---\ntitle: Meu texto\nauthor: Ana\n---\n\nOi\n");
        assert_eq!(d.meta.titulo, "Meu texto");
        assert_eq!(d.meta.autor, "Ana");
        assert_eq!(d.blocos.len(), 1);
    }

    #[test]
    fn imagem_e_link() {
        let d = ler("Veja [aqui](http://x.org).\n\n![Legenda](a.png)\n");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        assert_eq!(texto[1].link.as_deref(), Some("http://x.org"));
        assert!(matches!(&d.blocos[1].conteudo, Conteudo::Imagem { origem, .. } if origem == "a.png"));
    }

    #[test]
    fn notas_de_rodape_e_de_fim_vao_e_voltam() {
        let d = ler("Texto[^1] e mais[^fim-1].\n\n[^1]: A **nota**.\n[^fim-1]: Do fim.\n");
        assert_eq!(d.blocos.len(), 1, "as definições não viram parágrafo");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        let notas: Vec<(bool, String)> = texto.iter().filter_map(|t| match t.marca.as_deref() {
            Some(Marca::Nota { fim, texto }) => Some((*fim, featherdocs_core::inline::serializar(texto))),
            _ => None,
        }).collect();
        assert_eq!(notas, vec![(false, "A **nota**.".to_string()), (true, "Do fim.".to_string())]);
        let md = crate::exportar::markdown(&d);
        assert!(md.contains("Texto[^1] e mais[^fim-1]."), "{md}");
        assert!(md.contains("[^1]: A **nota**.") && md.contains("[^fim-1]: Do fim."), "{md}");
        assert_eq!(ler(&md), d);
        let tex = crate::exportar::latex(&d);
        assert!(tex.contains("Texto\\footnote{A \\textbf{nota}.} e mais\\endnote{Do fim.}"), "{tex}");
        assert!(tex.contains("\\usepackage{endnotes}") && tex.contains("\\theendnotes"));
    }

    #[test]
    fn citacoes_fontes_e_listas_geradas_vao_e_voltam() {
        use featherdocs_core::modelo::{Autor, EstiloDeCitacao, Fonte};
        let d = ler("Segundo [@silva2023, p. 4] e [@a; @b].\n\n[REFERENCIAS]\n\n[INDICE]\n");
        let Conteudo::Paragrafo { texto } = &d.blocos[0].conteudo else { panic!() };
        let m: Vec<&Marca> = texto.iter().filter_map(|t| t.marca.as_deref()).collect();
        assert!(matches!(m[0], Marca::Citacao { chaves, pagina } if chaves == &["silva2023"] && pagina == "4"), "{m:?}");
        assert!(matches!(m[1], Marca::Citacao { chaves, .. } if chaves.len() == 2));
        assert_eq!(texto_puro(texto), "Segundo  e .");
        assert!(matches!(d.blocos[1].conteudo, Conteudo::Bibliografia { todas: false }));
        assert!(matches!(d.blocos[2].conteudo, Conteudo::IndiceRemissivo));
        let mut d = d;
        d.bibliografia.estilo = EstiloDeCitacao::Vancouver;
        d.bibliografia.fontes.push(Fonte { chave: "silva2023".into(), autores: vec![Autor { sobrenome: "Silva".into(), nome: "Ana".into() }], titulo: "Um -- dois".into(), ano: "2023".into(), ..Fonte::default() });
        let md = crate::exportar::markdown(&d);
        let volta = ler(&md);
        assert_eq!(volta.bibliografia.estilo, EstiloDeCitacao::Vancouver);
        assert_eq!(volta.bibliografia.fontes.len(), 1, "{md}");
        assert_eq!(volta.blocos.len(), 3);
    }

    #[test]
    fn exportar_e_reler_da_o_mesmo_texto() {
        let md = "# Título\n\nUm **par** com `cod`.\n\n- a\n- b\n\n> citação\n";
        let d = ler(md);
        let volta = ler(&crate::exportar::markdown(&d));
        assert_eq!(tipos(md), volta.blocos.iter().map(|b| nome_do_tipo(&b.conteudo)).collect::<Vec<_>>());
        assert_eq!(crate::exportar::texto_simples(&d), crate::exportar::texto_simples(&volta));
    }
}
