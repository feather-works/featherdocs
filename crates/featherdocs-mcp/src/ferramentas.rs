//! As ferramentas `docs_*`: o esquema que a IA vê ([`esquemas`]) e o que cada uma faz ([`chamar`]).
use std::path::Path;

use featherdocs_core::modelo::{Bloco, Conteudo};
use featherdocs_core::sessao::Sessao;
use serde_json::{json, Value};

use crate::hospede::Hospede;

/// Teto de blocos por leitura: um documento longo não vai inteiro para a IA.
const MAXIMO_LER: usize = 120;
/// Teto de caracteres por bloco lido.
const MAXIMO_BLOCO: usize = 3000;
const MAXIMO_ACHADOS: usize = 50;

fn esquema(nome: &str, descricao: &str, propriedades: Value, obrigatorias: &[&str]) -> Value {
    json!({ "name": nome, "description": descricao,
        "inputSchema": { "type": "object", "properties": propriedades, "required": obrigatorias } })
}

pub fn esquemas() -> Vec<Value> {
    let inteiro = |d: &str| json!({ "type": "integer", "description": d });
    let texto = |d: &str| json!({ "type": "string", "description": d });
    vec![
        esquema("docs_resumo", "Título, autor, páginas, palavras, número de blocos e os títulos do documento com o índice do bloco de cada um.", json!({}), &[]),
        esquema("docs_ler", "Os blocos de..ate (índices, inclusive) em Markdown, cada um com o índice e o tipo. Teto de 120 blocos.",
            json!({ "de": inteiro("Primeiro bloco (0 = o começo)."), "ate": inteiro("Último bloco (padrão: de + 40).") }), &[]),
        esquema("docs_buscar", "Os blocos que contêm um texto (sem diferenciar maiúsculas), com o índice e um trecho.",
            json!({ "texto": texto("O texto procurado.") }), &["texto"]),
        esquema("docs_inserir", "Insere Markdown como blocos novos antes do bloco `posicao` (padrão: no fim). Um Ctrl+Z.",
            json!({ "markdown": texto("O conteúdo: # Título, parágrafos, - itens, | tabelas |, **negrito**."), "posicao": inteiro("Índice onde os blocos entram (0 = o começo).") }), &["markdown"]),
        esquema("docs_trocar", "Substitui os blocos de..ate (inclusive) pelo Markdown dado. Um Ctrl+Z.",
            json!({ "de": inteiro("Primeiro bloco trocado."), "ate": inteiro("Último bloco trocado (padrão: de)."), "markdown": texto("O conteúdo novo.") }), &["de", "markdown"]),
        esquema("docs_apagar", "Apaga os blocos de..ate (inclusive). Um Ctrl+Z.",
            json!({ "de": inteiro("Primeiro bloco."), "ate": inteiro("Último bloco (padrão: de).") }), &["de"]),
        esquema("docs_substituir", "Troca um texto por outro em todo o documento (mantém a formatação em volta). Um Ctrl+Z.",
            json!({ "procurar": texto("O texto procurado."), "por": texto("O texto novo."), "maiusculas": json!({ "type": "boolean", "description": "Diferenciar maiúsculas." }) }), &["procurar", "por"]),
        esquema("docs_desfazer", "Desfaz a última edição (como Ctrl+Z).", json!({}), &[]),
        esquema("docs_modelo", "Sem `id`, lista os modelos; com `id`, começa um documento novo a partir dele (o documento atual é trocado; com mudanças não salvas, só com `descartar`).",
            json!({ "id": texto("O id do modelo (ex.: carta, relatorio, timbrado)."), "descartar": json!({ "type": "boolean", "description": "Jogar fora as mudanças não salvas do documento atual." }) }), &[]),
        esquema("docs_metadados", "Define título e autor do documento (não entra no Ctrl+Z).", json!({ "titulo": texto("Título."), "autor": texto("Autor.") }), &[]),
        esquema("docs_abrir", "Abre um arquivo (.fdoz, .docx, .md, .tex, .zip do LaTeX). Com mudanças não salvas no documento atual, só com `descartar`.",
            json!({ "caminho": texto("Caminho do arquivo."), "descartar": json!({ "type": "boolean", "description": "Jogar fora as mudanças não salvas do documento atual." }) }), &["caminho"]),
        esquema("docs_salvar", "Grava o documento; a extensão escolhe o formato (.fdoz, .docx, .md, .tex, .zip). Sem `caminho`, no arquivo aberto. O caminho é absoluto; um arquivo que já existe (e não é o aberto) só com `sobrescrever`.",
            json!({ "caminho": texto("Destino (caminho absoluto)."), "sobrescrever": json!({ "type": "boolean", "description": "Substituir um arquivo que já existe." }) }), &[]),
    ]
}

fn numero(args: &Value, nome: &str) -> Option<usize> {
    args.get(nome).and_then(Value::as_u64).map(|n| n as usize)
}

fn texto<'a>(args: &'a Value, nome: &str) -> Result<&'a str, String> {
    args.get(nome).and_then(Value::as_str).ok_or_else(|| format!("Falta `{nome}`."))
}

fn tipo(c: &Conteudo) -> &'static str {
    match c {
        Conteudo::Titulo { .. } => "título",
        Conteudo::Paragrafo { .. } => "parágrafo",
        Conteudo::Item { .. } => "item",
        Conteudo::Citacao { .. } => "citação",
        Conteudo::Codigo { .. } => "código",
        Conteudo::Tabela { .. } => "tabela",
        Conteudo::Imagem { .. } => "imagem",
        Conteudo::Equacao { .. } => "equação",
        Conteudo::Sumario { .. } => "sumário",
        Conteudo::QuebraDePagina => "quebra de página",
        _ => "outro",
    }
}

/// O bloco em Markdown; a imagem vira uma descrição (nada de base64 para a IA).
fn markdown(c: &Conteudo) -> String {
    let t = match c {
        Conteudo::Imagem { legenda, largura, altura, .. } => format!("[imagem {largura:.0}×{altura:.0} pt; legenda: {}]", featherdocs_core::texto::plano(legenda)),
        _ => featherdocs_io::exportar::bloco_markdown(c),
    };
    let t = t.trim_end().to_string();
    match t.char_indices().nth(MAXIMO_BLOCO) {
        Some((i, _)) => format!("{}… (cortado)", &t[..i]),
        None => t,
    }
}

/// Os blocos de um texto em Markdown, com `id` 0 (ganham id novo na troca).
fn blocos_do_markdown(md: &str) -> Vec<Bloco> {
    let mut v = featherdocs_io::markdown::ler(md).blocos;
    for b in &mut v {
        b.id = 0;
    }
    v
}

/// O documento protegido (o `w:documentProtection` do Word) não aceita edição: a sessão a ignora
/// sem erro, e o agente acharia que editou.
fn editavel(h: &dyn Hospede) -> Result<(), String> {
    if h.sessao().bloqueado() {
        return Err("O documento está protegido contra edição (ou em modo só leitura): nada foi mudado.".into());
    }
    Ok(())
}

/// O `de..ate` (inclusive) de trocar e apagar, já como o fim exclusivo; `ate` antes de `de` é erro.
fn faixa(args: &Value) -> Result<(usize, usize), String> {
    let de = numero(args, "de").ok_or("Falta `de`.")?;
    let ate = numero(args, "ate").unwrap_or(de);
    if ate < de {
        return Err(format!("`ate` ({ate}) vem antes de `de` ({de})."));
    }
    Ok((de, ate.checked_add(1).ok_or("`ate` grande demais.")?))
}

/// Com mudanças não salvas, trocar de documento só com `"descartar": true`.
fn pode_trocar_de_documento(h: &dyn Hospede, args: &Value) -> Result<(), String> {
    if h.sessao().sujo() && !args.get("descartar").and_then(Value::as_bool).unwrap_or(false) {
        return Err("O documento atual tem mudanças não salvas: salve com docs_salvar ou repita com \"descartar\": true.".into());
    }
    Ok(())
}

/// Troca `blocos[de..ate]` por `novos` num passo de desfazer.
fn trocar(s: &mut Sessao, de: usize, ate: usize, novos: Vec<Bloco>) -> Result<(), String> {
    let mut todos = s.documento().blocos.clone();
    if de > todos.len() || ate > todos.len() || de > ate {
        return Err(format!("Índices fora do documento: ele tem {} bloco(s) (0 a {}).", todos.len(), todos.len().saturating_sub(1)));
    }
    todos.splice(de..ate, novos);
    s.trocar_blocos(todos, None, None);
    Ok(())
}

pub fn chamar(h: &mut dyn Hospede, nome: &str, args: &Value) -> Result<String, String> {
    match nome {
        "docs_resumo" => {
            let s = h.sessao();
            let d = s.documento();
            let c = s.contagem();
            let mut r = format!("Título: {}\nAutor: {}\nArquivo: {}\nPáginas: {} · palavras: {} · blocos: {}\n",
                d.meta.titulo, d.meta.autor, h.caminho().map_or("(sem arquivo)".into(), |p| p.display().to_string()),
                s.total_de_paginas(), c.palavras, d.blocos.len());
            let titulos = s.estrutura();
            if !titulos.is_empty() {
                r.push_str("Títulos (índice do bloco · página):\n");
                for t in titulos {
                    let i = d.indice(t.id).unwrap_or(0);
                    r.push_str(&format!("{}[{i}] {} · p. {}\n", "  ".repeat(usize::from(t.nivel.saturating_sub(1))), t.texto, t.pagina + 1));
                }
            }
            Ok(r)
        }
        "docs_ler" => {
            let blocos = &h.sessao().documento().blocos;
            let de = numero(args, "de").unwrap_or(0);
            let ate = numero(args, "ate").unwrap_or(de + 40).min(de + MAXIMO_LER - 1).min(blocos.len().saturating_sub(1));
            if de >= blocos.len() {
                return Err(format!("O documento tem {} bloco(s).", blocos.len()));
            }
            let mut r = String::new();
            for (i, b) in blocos.iter().enumerate().take(ate + 1).skip(de) {
                r.push_str(&format!("[{i}] ({}) {}\n", tipo(&b.conteudo), markdown(&b.conteudo)));
            }
            if ate + 1 < blocos.len() {
                r.push_str(&format!("… mais {} bloco(s); continue com de = {}.\n", blocos.len() - ate - 1, ate + 1));
            }
            Ok(r)
        }
        "docs_buscar" => {
            let alvo = texto(args, "texto")?.to_lowercase();
            if alvo.is_empty() {
                return Err("`texto` vazio.".into());
            }
            let mut r = String::new();
            let mut n = 0;
            for (i, b) in h.sessao().documento().blocos.iter().enumerate() {
                let t = markdown(&b.conteudo);
                if t.to_lowercase().contains(&alvo) {
                    n += 1;
                    if n <= MAXIMO_ACHADOS {
                        r.push_str(&format!("[{i}] {}\n", t.chars().take(160).collect::<String>()));
                    }
                }
            }
            Ok(if n == 0 { "Nada encontrado.".into() } else { format!("{n} bloco(s):\n{r}") })
        }
        "docs_inserir" => {
            editavel(h)?;
            let novos = blocos_do_markdown(texto(args, "markdown")?);
            let total = h.sessao().documento().blocos.len();
            let pos = numero(args, "posicao").unwrap_or(total).min(total);
            let n = novos.len();
            trocar(h.sessao_mut(), pos, pos, novos)?;
            Ok(format!("{n} bloco(s) inseridos a partir do índice {pos}."))
        }
        "docs_trocar" => {
            editavel(h)?;
            let (de, fim) = faixa(args)?;
            let novos = blocos_do_markdown(texto(args, "markdown")?);
            let n = novos.len();
            trocar(h.sessao_mut(), de, fim, novos)?;
            Ok(format!("Blocos {de}..{} trocados por {n} bloco(s).", fim - 1))
        }
        "docs_apagar" => {
            editavel(h)?;
            let (de, fim) = faixa(args)?;
            trocar(h.sessao_mut(), de, fim, Vec::new())?;
            Ok(format!("Blocos {de}..{} apagados.", fim - 1))
        }
        "docs_substituir" => {
            editavel(h)?;
            let procurar = texto(args, "procurar")?;
            let por = texto(args, "por")?;
            let opcoes = featherdocs_core::busca::Opcoes { maiusculas: args.get("maiusculas").and_then(Value::as_bool).unwrap_or(false), ..Default::default() };
            let s = h.sessao_mut();
            let r = s.busca_definir(procurar, opcoes);
            if let Some(e) = r.erro {
                return Err(e);
            }
            let (_, n) = s.substituir_tudo(por);
            s.busca_limpar();
            Ok(format!("{n} troca(s)."))
        }
        "docs_desfazer" => Ok(if h.sessao_mut().desfazer().is_some() { "Desfeito.".into() } else { "Nada para desfazer.".into() }),
        "docs_modelo" => match args.get("id").and_then(Value::as_str) {
            None => Ok(featherdocs_core::modelos::lista().iter().map(|m| format!("{} — {}: {}", m.id, m.nome, m.descricao)).collect::<Vec<_>>().join("\n")),
            Some(id) => {
                pode_trocar_de_documento(h, args)?;
                let d = featherdocs_core::modelos::criar(id).ok_or_else(|| format!("Modelo `{id}` não existe; chame docs_modelo sem id para a lista."))?;
                h.novo(Sessao::com_documento(d));
                Ok(format!("Documento novo do modelo `{id}`: {} bloco(s). Leia com docs_ler.", h.sessao().documento().blocos.len()))
            }
        },
        "docs_metadados" => {
            editavel(h)?;
            let d = h.sessao().documento();
            let titulo = args.get("titulo").and_then(Value::as_str).unwrap_or(&d.meta.titulo).to_string();
            let autor = args.get("autor").and_then(Value::as_str).unwrap_or(&d.meta.autor).to_string();
            h.sessao_mut().meta_definir(&titulo, &autor);
            Ok("Título e autor definidos.".into())
        }
        "docs_abrir" => {
            pode_trocar_de_documento(h, args)?;
            h.abrir(Path::new(texto(args, "caminho")?))
        }
        "docs_salvar" => {
            let destino = args.get("caminho").and_then(Value::as_str).map(Path::new);
            if let Some(p) = destino {
                // Relativo cairia na pasta onde o servidor rodou (o projeto do cliente).
                if !p.is_absolute() {
                    return Err("Use um caminho absoluto para `caminho`.".into());
                }
                let o_aberto = h.caminho().is_some_and(|c| c == p);
                if p.exists() && !o_aberto && !args.get("sobrescrever").and_then(Value::as_bool).unwrap_or(false) {
                    return Err(format!("{} já existe: repita com \"sobrescrever\": true para substituí-lo.", p.display()));
                }
            }
            h.salvar(destino)
        }
        _ => Err(format!("Ferramenta desconhecida: {nome}")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hospede::Avulso;

    fn ok(h: &mut Avulso, nome: &str, args: Value) -> String {
        chamar(h, nome, &args).unwrap_or_else(|e| panic!("{nome}: {e}"))
    }

    #[test]
    fn escreve_le_troca_desfaz_e_grava() {
        let mut h = Avulso::default();
        ok(&mut h, "docs_trocar", json!({ "de": 0, "markdown": "# Plano\n\nTexto **forte**.\n\n- um\n- dois" }));
        let lido = ok(&mut h, "docs_ler", json!({}));
        assert!(lido.contains("[0] (título) # Plano") && lido.contains("**forte**") && lido.contains("(item)"), "{lido}");
        assert!(ok(&mut h, "docs_resumo", json!({})).contains("[0] Plano"));
        ok(&mut h, "docs_inserir", json!({ "markdown": "Antes de tudo.", "posicao": 0 }));
        assert!(ok(&mut h, "docs_ler", json!({ "de": 0, "ate": 0 })).contains("Antes de tudo."));
        assert_eq!(ok(&mut h, "docs_substituir", json!({ "procurar": "dois", "por": "três" })), "1 troca(s).");
        assert!(ok(&mut h, "docs_buscar", json!({ "texto": "TRÊS" })).starts_with("1 bloco"));
        ok(&mut h, "docs_desfazer", json!({}));
        assert!(ok(&mut h, "docs_buscar", json!({ "texto": "dois" })).starts_with("1 bloco"), "desfazer volta a troca");
        ok(&mut h, "docs_apagar", json!({ "de": 0 }));
        assert!(ok(&mut h, "docs_ler", json!({ "de": 0, "ate": 0 })).contains("Plano"));
        assert!(chamar(&mut h, "docs_apagar", &json!({ "de": 99 })).is_err());
        let pasta = std::env::temp_dir().join(format!("featherdocs-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        for ext in ["fdoz", "docx", "md"] {
            let p = pasta.join(format!("teste.{ext}"));
            ok(&mut h, "docs_salvar", json!({ "caminho": p.to_str().unwrap() }));
            let mut outro = Avulso::default();
            ok(&mut outro, "docs_abrir", json!({ "caminho": p.to_str().unwrap() }));
            assert!(ok(&mut outro, "docs_buscar", json!({ "texto": "Plano" })).starts_with("1 bloco"), "{ext} volta");
        }
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn indices_errados_protecao_e_gravacao_sao_recusados() {
        let mut h = Avulso::default();
        ok(&mut h, "docs_trocar", json!({ "de": 0, "markdown": "um\n\ndois\n\ntres" }));
        for args in [json!({ "de": 2, "ate": 1 }), json!({ "de": 0, "ate": u64::MAX }), json!({ "de": 3 })] {
            assert!(chamar(&mut h, "docs_apagar", &args).is_err(), "{args}");
            assert!(chamar(&mut h, "docs_trocar", &json!({ "de": args["de"], "ate": args["ate"], "markdown": "x" })).is_err(), "{args}");
        }
        assert_eq!(h.sessao().documento().blocos.len(), 3);
        // Sujo: trocar de documento só com `descartar`.
        assert!(chamar(&mut h, "docs_modelo", &json!({ "id": "carta" })).is_err());
        assert!(chamar(&mut h, "docs_salvar", &json!({ "caminho": "relativo.md" })).is_err(), "caminho relativo");
        let pasta = std::env::temp_dir().join(format!("featherdocs-mcp-recusa-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let outro = pasta.join("outro.md");
        std::fs::write(&outro, "não é meu").unwrap();
        assert!(chamar(&mut h, "docs_salvar", &json!({ "caminho": outro.to_str().unwrap() })).is_err(), "não sobrescreve sem pedir");
        ok(&mut h, "docs_salvar", json!({ "caminho": outro.to_str().unwrap(), "sobrescrever": true }));
        // O .fdoz salvo duas vezes guarda as duas versões.
        let fdoz = pasta.join("doc.fdoz");
        ok(&mut h, "docs_salvar", json!({ "caminho": fdoz.to_str().unwrap() }));
        ok(&mut h, "docs_inserir", json!({ "markdown": "quatro" }));
        ok(&mut h, "docs_salvar", json!({}));
        assert_eq!(featherdocs_historico::versoes(&std::fs::read(&fdoz).unwrap()).unwrap().len(), 2);
        ok(&mut h, "docs_modelo", json!({ "id": "carta" }));
        // Protegido: a edição é recusada, não fingida.
        h.sessao_mut().proteger(true);
        assert!(chamar(&mut h, "docs_inserir", &json!({ "markdown": "x" })).is_err());
        let _ = std::fs::remove_dir_all(pasta);
        assert!(crate::atender(&mut h, &json!({ "jsonrpc": "2.0", "id": 7 }))
            .is_some_and(|r| r["error"]["code"] == -32600));
    }

    #[test]
    fn modelos_e_protocolo() {
        let mut h = Avulso::default();
        assert!(ok(&mut h, "docs_modelo", json!({})).contains("timbrado"));
        ok(&mut h, "docs_modelo", json!({ "id": "relatorio" }));
        let lido = ok(&mut h, "docs_ler", json!({ "de": 0, "ate": 200 }));
        assert!(!lido.contains("base64"), "imagem não vai em base64");
        for f in esquemas() {
            let nome = f["name"].as_str().unwrap();
            let r = crate::chamar_ferramenta(&mut h, &json!({ "name": nome, "arguments": {} }));
            assert!(!r["content"][0]["text"].as_str().unwrap_or("").contains("desconhecida"), "{nome}");
        }
        let init = crate::atender(&mut h, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "featherdocs");
    }
}
