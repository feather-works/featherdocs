//! A ponte entre o `featherdocs-mcp` (que a IA abre pelo stdio) e o aplicativo aberto na tela: com a
//! ponte ligada no app, as ferramentas agem no documento da tela (cada edição aparece na hora e é
//! um Ctrl+Z). Só no próprio computador: escuta em 127.0.0.1 e exige a senha que o app grava em
//! `ponte.json`, na pasta de dados do usuário (só ele lê o arquivo).
//!
//! O protocolo é uma linha JSON por pedido: `{"senha": "…", "params": {…}}` (os `params` do
//! `tools/call`), e uma linha com o resultado da ferramenta.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use serde_json::{json, Value};

/// Onde o app diz a porta e a senha: a mesma pasta de dados da recuperação.
pub fn arquivo() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    Some(base.join("featherdocs/ponte.json"))
}

/// Uma senha de 128 bits do gerador do sistema (o `RandomState` da biblioteca padrão é semeado por ele).
fn senha() -> String {
    use std::hash::{BuildHasher, Hasher};
    let parte = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.finish()
    };
    format!("{:016x}{:016x}", parte(), parte())
}

/// Um pedido que chegou da IA: os `params` do `tools/call` e por onde vai a resposta.
pub struct Pedido {
    pub params: Value,
    resposta: Sender<Value>,
}

impl Pedido {
    pub fn responder(self, resultado: Value) {
        let _ = self.resposta.send(resultado);
    }
}

/// O lado do app: escuta numa porta local e entrega os pedidos para a thread da tela.
pub struct Servidor {
    pedidos: Receiver<Pedido>,
    arquivo: PathBuf,
}

impl Servidor {
    /// Começa a escutar e grava `ponte.json` (porta e senha) em `arquivo`.
    pub fn iniciar(arquivo: PathBuf) -> std::io::Result<Self> {
        let escuta = TcpListener::bind("127.0.0.1:0")?;
        let porta = escuta.local_addr()?.port();
        let senha = senha();
        if let Some(pasta) = arquivo.parent() {
            std::fs::create_dir_all(pasta)?;
        }
        gravar_privado(&arquivo, json!({ "porta": porta, "senha": senha }).to_string().as_bytes())?;
        let (tx, pedidos) = channel();
        std::thread::spawn(move || {
            for conexao in escuta.incoming().flatten() {
                let (tx, senha) = (tx.clone(), senha.clone());
                std::thread::spawn(move || atender(conexao, &senha, &tx));
            }
        });
        Ok(Self { pedidos, arquivo })
    }

    /// Os pedidos que chegaram desde a última vez (o app os atende e responde cada um).
    pub fn pedidos(&self) -> Vec<Pedido> {
        self.pedidos.try_iter().collect()
    }
}

impl Drop for Servidor {
    /// Ponte desligada (ou app fechado): o `featherdocs-mcp` volta a trabalhar sobre arquivos.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.arquivo);
    }
}

fn gravar_privado(p: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(p)?.write_all(bytes)
}

/// Uma conexão do `featherdocs-mcp`: cada linha é um pedido; a senha errada fecha a conexão.
fn atender(conexao: TcpStream, senha: &str, tx: &Sender<Pedido>) {
    let Ok(mut saida) = conexao.try_clone() else { return };
    for linha in BufReader::new(conexao).lines() {
        let Ok(linha) = linha else { return };
        let Ok(msg) = serde_json::from_str::<Value>(&linha) else { return };
        if msg.get("senha").and_then(Value::as_str) != Some(senha) {
            return;
        }
        let (resposta, recebe) = channel();
        if tx.send(Pedido { params: msg.get("params").cloned().unwrap_or(Value::Null), resposta }).is_err() {
            return;
        }
        // O app atende na próxima volta da tela; sem resposta em um minuto, a ferramenta dá erro.
        let r = recebe.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| erro("o FeatherDocs não respondeu"));
        if writeln!(saida, "{r}").and_then(|()| saida.flush()).is_err() {
            return;
        }
    }
}

fn erro(texto: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": texto }], "isError": true })
}

/// O lado do `featherdocs-mcp`: a conexão com o app, se ele está com a ponte ligada.
pub struct Cliente {
    senha: String,
    escrita: TcpStream,
    leitura: BufReader<TcpStream>,
}

impl Cliente {
    /// Liga no app pelo `ponte.json`; `None` se o app não está aberto com a ponte.
    pub fn conectar(arquivo: &std::path::Path) -> Option<Self> {
        let dados: Value = serde_json::from_slice(&std::fs::read(arquivo).ok()?).ok()?;
        let porta = u16::try_from(dados.get("porta")?.as_u64()?).ok()?;
        let senha = dados.get("senha")?.as_str()?.to_string();
        let escrita = TcpStream::connect_timeout(&([127, 0, 0, 1], porta).into(), Duration::from_secs(2)).ok()?;
        escrita.set_read_timeout(Some(Duration::from_secs(90))).ok()?;
        let leitura = BufReader::new(escrita.try_clone().ok()?);
        Some(Self { senha, escrita, leitura })
    }

    /// Manda o `tools/call` ao app e devolve o resultado da ferramenta.
    pub fn chamar(&mut self, params: &Value) -> Result<Value, String> {
        let pedido = json!({ "senha": self.senha, "params": params });
        writeln!(self.escrita, "{pedido}").and_then(|()| self.escrita.flush()).map_err(|e| e.to_string())?;
        let mut linha = String::new();
        if self.leitura.read_line(&mut linha).map_err(|e| e.to_string())? == 0 {
            return Err("o FeatherDocs fechou a ponte".into());
        }
        serde_json::from_str(&linha).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pedido_vai_ao_app_e_a_resposta_volta_e_senha_errada_nao_passa() {
        let arquivo = std::env::temp_dir().join(format!("featherdocs-ponte-{}.json", std::process::id()));
        let servidor = Servidor::iniciar(arquivo.clone()).unwrap();
        // A "tela" do app: atende os pedidos que chegam.
        let tela = std::thread::spawn(move || {
            let mut atendidos = 0;
            while atendidos < 1 {
                for p in servidor.pedidos() {
                    let nome = p.params["name"].as_str().unwrap_or("").to_string();
                    p.responder(json!({ "content": [{ "type": "text", "text": format!("feito: {nome}") }], "isError": false }));
                    atendidos += 1;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            servidor
        });
        let mut c = Cliente::conectar(&arquivo).expect("o app está com a ponte");
        let r = c.chamar(&json!({ "name": "docs_resumo", "arguments": {} })).unwrap();
        assert_eq!(r["content"][0]["text"], "feito: docs_resumo");

        c.senha = "errada".into();
        assert!(c.chamar(&json!({ "name": "docs_ler" })).is_err(), "a senha errada fecha a conexão");

        drop(tela.join().unwrap());
        assert!(!arquivo.exists(), "desligada, o ponte.json some");
        assert!(Cliente::conectar(&arquivo).is_none());
    }
}
