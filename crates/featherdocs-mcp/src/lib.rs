//! O servidor MCP do FeatherDocs (JSON-RPC 2.0, uma mensagem por linha no stdio): `initialize`,
//! `tools/list`, `tools/call` e `ping`. As ferramentas agem sobre um [`hospede::Hospede`].
//!
//! O texto entra e sai em Markdown (títulos `#`, listas, tabelas, **negrito**), e os blocos são
//! apontados pelo índice que `docs_ler` mostra. Cada edição é um passo de Ctrl+Z (`trocar_blocos`).
pub mod ferramentas;
pub mod hospede;

use serde_json::{json, Value};

const VERSOES: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const INSTRUCOES: &str = "FeatherDocs: processador de texto local. Comece por docs_resumo (títulos e tamanho) e leia em pedaços com docs_ler (índices de bloco). O texto entra e sai em Markdown: # título, - item, 1. item, | tabela |, **negrito**, *itálico*. Para escrever use docs_inserir (blocos novos numa posição) ou docs_trocar (substitui os blocos de..ate); docs_substituir troca um texto em todo o documento. Cada edição é um Ctrl+Z (docs_desfazer), menos o título e o autor (docs_metadados). Os índices mudam depois de inserir ou apagar: leia de novo antes da próxima edição. Sem o aplicativo, abra com docs_abrir (ou comece de um modelo com docs_modelo) e grave com docs_salvar (.fdoz, .docx, .md, .tex).";

fn resposta(id: &Value, resultado: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": resultado })
}

fn inicializar(params: &Value) -> Value {
    let pedida = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSOES[0]);
    let versao = if VERSOES.contains(&pedida) { pedida } else { VERSOES[0] };
    json!({ "protocolVersion": versao, "capabilities": { "tools": {} },
        "serverInfo": { "name": "featherdocs", "version": env!("CARGO_PKG_VERSION") }, "instructions": INSTRUCOES })
}

/// Erro de ferramenta volta como resultado com `isError`; um pânico não derruba o servidor.
pub fn chamar_ferramenta(h: &mut dyn hospede::Hospede, params: &Value) -> Value {
    let nome = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ferramentas::chamar(h, nome, &args))) {
        Ok(Ok(texto)) => json!({ "content": [{ "type": "text", "text": texto }], "isError": false }),
        Ok(Err(e)) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
        Err(_) => json!({ "content": [{ "type": "text", "text": format!("erro interno na ferramenta `{nome}`. Confira o documento (docs_ler) antes de repetir.") }], "isError": true }),
    }
}

/// Atende uma mensagem. `None` para notificações (sem id).
pub fn atender(h: &mut dyn hospede::Hospede, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?;
    let Some(metodo) = msg.get("method").and_then(Value::as_str) else {
        return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32600, "message": "pedido sem `method`" } }));
    };
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    Some(match metodo {
        "initialize" => resposta(id, inicializar(&params)),
        "ping" => resposta(id, json!({})),
        "tools/list" => resposta(id, json!({ "tools": ferramentas::esquemas() })),
        "prompts/list" => resposta(id, json!({ "prompts": [] })),
        "tools/call" => resposta(id, chamar_ferramenta(h, &params)),
        outro => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("método desconhecido: {outro}") } }),
    })
}

/// O servidor por stdio: o `featherdocs-mcp` (sem o aplicativo, sobre arquivos).
pub fn rodar_stdio() {
    use std::io::{BufRead, Write};
    let mut h = hospede::Avulso::default();
    let saida = std::io::stdout();
    for linha in std::io::stdin().lock().lines() {
        let Ok(linha) = linha else { break };
        if linha.trim().is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Value>(&linha) {
            Ok(msg) => atender(&mut h, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("JSON inválido: {e}") } })),
        };
        if let Some(r) = r {
            let mut s = saida.lock();
            let _ = writeln!(s, "{r}");
            let _ = s.flush();
        }
    }
}
