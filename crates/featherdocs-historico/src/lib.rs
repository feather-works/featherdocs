//! O histórico de versões dentro do `.fdoz`: um repositório Git **embutido** (contrato em
//! `FORMATO.md`), em Rust puro com o `gix`.
//!
//! O `.fdoz` que o motor grava (`data/*.arrow`, `assets/`) não sabe de Git. Ao salvar no disco, o
//! aplicativo passa por [`gravar_com_historico`]: o `.git/` do arquivo anterior é desempacotado numa
//! pasta temporária, os arquivos do documento novo viram uma árvore Git e um commit novo (se algo
//! mudou), e tudo volta para dentro do ZIP em `.git/`. [`versoes`] é o `git log` do arquivo e
//! [`abrir_versao`] remonta o `.fdoz` de uma versão antiga.
//!
//! Nada sai da máquina: não há `remote`, nem rede.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use gix::bstr::{BString, ByteSlice};
use gix::objs::tree::{Entry, EntryKind};
use gix::ObjectId;
use zip::write::SimpleFileOptions;

const PASTA_GIT: &str = ".git/";
/// Teto do que uma parte do ZIP pode ocupar descompactada, e do arquivo todo: o tamanho declarado
/// no ZIP pode mentir (a "bomba de ZIP"), então o teto vale para o que sai de fato.
const LIMITE_PARTE: u64 = if cfg!(test) { 1 << 20 } else { 256 << 20 };
const LIMITE_TOTAL: u64 = 4 * LIMITE_PARTE;
/// Passou disto, o `.git/` perde a metade mais velha das versões (senão o arquivo só cresce).
const LIMITE_HISTORICO: u64 = if cfg!(test) { 64 << 10 } else { 64 << 20 };

/// Uma versão do documento (um commit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Versao {
    /// O hash do commit, em hexadecimal.
    pub id: String,
    /// Segundos desde 1970 (UTC) e o fuso de quem salvou, em segundos.
    pub quando: i64,
    pub fuso: i32,
    pub autor: String,
    pub mensagem: String,
}

fn erro(e: impl std::fmt::Display) -> String {
    format!("Histórico de versões: {e}")
}

// ---------------------------------------------------------------------------
// ZIP e pasta temporária
// ---------------------------------------------------------------------------

/// As partes de um ZIP, na ordem: `(nome, bytes)`.
fn partes(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Isto não é um .fdoz válido: {e}"))?;
    let mut saida = Vec::with_capacity(zip.len());
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(erro)?;
        if f.is_dir() {
            continue;
        }
        let nome = f.name().to_string();
        let mut v = Vec::new();
        (&mut f).take(LIMITE_PARTE + 1).read_to_end(&mut v).map_err(erro)?;
        total += v.len() as u64;
        if v.len() as u64 > LIMITE_PARTE || total > LIMITE_TOTAL {
            return Err(erro(format!("{nome} passa do tamanho seguro descompactado")));
        }
        saida.push((nome, v));
    }
    Ok(saida)
}

fn zipar(partes: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // Os objetos do Git já vêm comprimidos (zlib): guardar de novo não ganha nada.
    let guardado = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (nome, bytes) in partes {
        let o = if nome.starts_with(".git/objects/") { guardado } else { opcoes };
        zip.start_file(nome.as_str(), o).map_err(erro)?;
        zip.write_all(bytes).map_err(erro)?;
    }
    Ok(zip.finish().map_err(erro)?.into_inner())
}

/// Uma pasta temporária que se apaga sozinha.
struct Temporaria(PathBuf);

impl Temporaria {
    fn nova() -> Result<Self, String> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        // O número em sequência: no Windows o relógio anda de 100 ns e duas pastas seguidas teriam o mesmo nome.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("featherdocs-historico-{}-{nanos}-{seq}", std::process::id()));
        // `create_dir`, não `_all`: uma pasta com esse nome já criada por outro (no /tmp de todos) é erro, não é usada.
        std::fs::create_dir(&p).map_err(erro)?;
        Ok(Self(p))
    }
}

impl Drop for Temporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// O repositório Git do `.fdoz` numa pasta temporária: o `.git/` de dentro do ZIP ou, sem ele, um novo.
fn repositorio(fdoz: Option<&[(String, Vec<u8>)]>, pasta: &Path) -> Result<gix::Repository, String> {
    let mut tem = false;
    for (nome, bytes) in fdoz.into_iter().flatten() {
        let Some(rel) = nome.strip_prefix(PASTA_GIT) else { continue };
        // Só nomes comuns: nada de `..`, raiz, `C:` nem `\` (que no Windows também separa pastas).
        let comum = Path::new(rel).components().all(|c| matches!(c, std::path::Component::Normal(_)));
        if rel.is_empty() || !comum || rel.contains(['\\', ':']) {
            return Err(erro(format!("caminho inválido dentro do .git: {nome}")));
        }
        let destino = pasta.join(rel);
        if let Some(pai) = destino.parent() {
            std::fs::create_dir_all(pai).map_err(erro)?;
        }
        std::fs::write(&destino, bytes).map_err(erro)?;
        tem = true;
    }
    if tem {
        gix::open(pasta).map_err(erro)
    } else {
        gix::init_bare(pasta).map_err(erro)
    }
}

/// Os arquivos de uma pasta (o `.git/`), com o caminho relativo, em ordem.
fn arquivos_da_pasta(raiz: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut saida = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(p) = pilha.pop() {
        for e in std::fs::read_dir(&p).map_err(erro)? {
            let e = e.map_err(erro)?;
            let caminho = e.path();
            if caminho.is_dir() {
                pilha.push(caminho);
            } else {
                let rel = caminho.strip_prefix(raiz).map_err(erro)?.to_string_lossy().replace('\\', "/");
                saida.push((format!("{PASTA_GIT}{rel}"), std::fs::read(&caminho).map_err(erro)?));
            }
        }
    }
    saida.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(saida)
}

// ---------------------------------------------------------------------------
// Árvores
// ---------------------------------------------------------------------------

/// Grava a árvore Git de uma lista de arquivos (`data/x.arrow`, `assets/y.png`…) e devolve o id.
fn gravar_arvore(repo: &gix::Repository, arquivos: &[(&str, &[u8])]) -> Result<ObjectId, String> {
    let mut entradas: Vec<Entry> = Vec::new();
    let mut subpastas: std::collections::BTreeMap<&str, Vec<(&str, &[u8])>> = std::collections::BTreeMap::new();
    for &(nome, bytes) in arquivos {
        match nome.split_once('/') {
            Some((pasta, resto)) => subpastas.entry(pasta).or_default().push((resto, bytes)),
            None => {
                let oid = repo.write_blob(bytes).map_err(erro)?.detach();
                entradas.push(Entry { mode: EntryKind::Blob.into(), filename: nome.into(), oid });
            }
        }
    }
    for (pasta, dentro) in subpastas {
        let oid = gravar_arvore(repo, &dentro)?;
        entradas.push(Entry { mode: EntryKind::Tree.into(), filename: pasta.into(), oid });
    }
    // A ordem do Git: pelo nome, com a pasta comparada como "nome/".
    let chave = |e: &Entry| {
        let mut k: BString = e.filename.clone();
        if e.mode.is_tree() {
            k.push(b'/');
        }
        k
    };
    entradas.sort_by_key(chave);
    Ok(repo.write_object(gix::objs::Tree { entries: entradas }).map_err(erro)?.detach())
}

/// Os arquivos de uma árvore Git, com o caminho completo.
fn ler_arvore(repo: &gix::Repository, id: ObjectId, prefixo: &str, saida: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    let arvore = repo.find_tree(id).map_err(erro)?;
    let decodificada = arvore.decode().map_err(erro)?;
    for e in &decodificada.entries {
        let nome = format!("{prefixo}{}", e.filename.to_str_lossy());
        if e.mode.is_tree() {
            ler_arvore(repo, e.oid.to_owned(), &format!("{nome}/"), saida)?;
        } else {
            saida.push((nome, repo.find_blob(e.oid.to_owned()).map_err(erro)?.data.clone()));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// O que o aplicativo chama
// ---------------------------------------------------------------------------

/// O `.fdoz` novo (sem Git, como o motor grava) com o histórico do arquivo `anterior` e um commit
/// a mais. Se o documento não mudou desde a última versão, não cria commit vazio. Passou de
/// [`LIMITE_HISTORICO`], a metade mais velha das versões sai.
pub fn gravar_com_historico(novo: &[u8], anterior: Option<&[u8]>, autor: &str, mensagem: &str) -> Result<Vec<u8>, String> {
    let documento: Vec<(String, Vec<u8>)> = partes(novo)?.into_iter().filter(|(n, _)| !n.starts_with(PASTA_GIT)).collect();
    let velho = anterior.map(partes).transpose()?;
    let tmp = Temporaria::nova()?;
    let repo = repositorio(velho.as_deref(), &tmp.0)?;
    let arquivos: Vec<(&str, &[u8])> = documento.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let arvore = gravar_arvore(&repo, &arquivos)?;
    let pai = repo.head_id().ok().map(gix::Id::detach);
    let mesma = pai.and_then(|p| repo.find_commit(p).ok()).and_then(|c| c.tree_id().ok()).is_some_and(|t| t == arvore);
    if !mesma {
        let nome = if autor.trim().is_empty() { "FeatherDocs" } else { autor.trim() };
        let assinatura = gix::actor::Signature { name: nome.into(), email: BString::default(), time: gix::date::Time::now_local_or_utc() };
        let mut buf = gix::date::parse::TimeBuf::default();
        let r = assinatura.to_ref(&mut buf);
        repo.commit_as(r, r, "HEAD", mensagem, arvore, pai).map_err(erro)?;
    }
    let mut git = arquivos_da_pasta(&tmp.0)?;
    let podado = Temporaria::nova()?;
    if git.iter().map(|(_, b)| b.len() as u64).sum::<u64>() > LIMITE_HISTORICO {
        podar(&repo, &podado.0)?;
        git = arquivos_da_pasta(&podado.0)?;
    }
    let mut tudo = documento;
    tudo.extend(git);
    zipar(&tudo)
}

/// Como [`gravar_com_historico`], com o autor do sistema (`USER`/`USERNAME`). Histórico anterior
/// danificado ou adulterado: começa um novo em vez de não salvar, e o `true` avisa quem chamou.
pub fn gravar_versao(novo: &[u8], anterior: Option<&[u8]>, mensagem: &str) -> Result<(Vec<u8>, bool), String> {
    let autor = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    match gravar_com_historico(novo, anterior, &autor, mensagem) {
        Ok(b) => Ok((b, false)),
        Err(_) if anterior.is_some() => gravar_com_historico(novo, None, &autor, mensagem).map(|b| (b, true)),
        Err(e) => Err(e),
    }
}

/// Os commits do HEAD para trás, do mais novo para o mais velho.
fn commits(repo: &gix::Repository) -> Result<Vec<ObjectId>, String> {
    let mut saida = Vec::new();
    let mut atual = repo.head_id().ok().map(gix::Id::detach);
    while let Some(id) = atual {
        saida.push(id);
        atual = repo.find_commit(id).map_err(erro)?.parent_ids().next().map(gix::Id::detach);
    }
    Ok(saida)
}

/// Refaz em `pasta` um repositório só com a metade mais nova das versões (ao menos uma), com o
/// mesmo autor, hora e mensagem; os objetos das versões que saíram não vão junto.
fn podar(repo: &gix::Repository, pasta: &Path) -> Result<(), String> {
    let ids = commits(repo)?;
    let novo = gix::init_bare(pasta).map_err(erro)?;
    let mut pai: Option<ObjectId> = None;
    for &id in ids[..ids.len().div_ceil(2)].iter().rev() {
        let c = repo.find_commit(id).map_err(erro)?;
        let mut arquivos = Vec::new();
        ler_arvore(repo, c.tree_id().map_err(erro)?.detach(), "", &mut arquivos)?;
        let refs: Vec<(&str, &[u8])> = arquivos.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let arvore = gravar_arvore(&novo, &refs)?;
        let mensagem = c.message_raw_sloppy().to_str_lossy().into_owned();
        pai = Some(novo.commit_as(c.committer().map_err(erro)?, c.author().map_err(erro)?, "HEAD", mensagem, arvore, pai).map_err(erro)?.detach());
    }
    Ok(())
}

/// O `git log` do `.fdoz`: as versões, da mais nova para a mais antiga. Sem histórico, vazio.
pub fn versoes(fdoz: &[u8]) -> Result<Vec<Versao>, String> {
    let todas = partes(fdoz)?;
    if !todas.iter().any(|(n, _)| n.starts_with(PASTA_GIT)) {
        return Ok(Vec::new());
    }
    let tmp = Temporaria::nova()?;
    let repo = repositorio(Some(&todas), &tmp.0)?;
    let mut saida = Vec::new();
    for id in commits(&repo)? {
        let c = repo.find_commit(id).map_err(erro)?;
        let autor = c.author().map_err(erro)?;
        let tempo = autor.time().map_err(erro)?;
        saida.push(Versao { id: id.to_string(), quando: tempo.seconds, fuso: tempo.offset, autor: autor.name.to_str_lossy().into_owned(), mensagem: c.message_raw_sloppy().to_str_lossy().trim().to_string() });
    }
    Ok(saida)
}

/// O `.fdoz` (sem o histórico) de uma versão antiga, pelo hash do commit.
pub fn abrir_versao(fdoz: &[u8], id: &str) -> Result<Vec<u8>, String> {
    let todas = partes(fdoz)?;
    let tmp = Temporaria::nova()?;
    let repo = repositorio(Some(&todas), &tmp.0)?;
    let oid = ObjectId::from_hex(id.as_bytes()).map_err(|_| erro(format!("versão inválida: {id}")))?;
    let commit = repo.find_commit(oid).map_err(|_| erro("essa versão não existe neste arquivo"))?;
    let mut arquivos = Vec::new();
    ler_arvore(&repo, commit.tree_id().map_err(erro)?.detach(), "", &mut arquivos)?;
    zipar(&arquivos)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn fdoz(partes: &[(&str, &[u8])]) -> Vec<u8> {
        zipar(&partes.iter().map(|(n, b)| (n.to_string(), b.to_vec())).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn cada_salvamento_vira_versao_e_cada_versao_volta() {
        let v1 = gravar_com_historico(&fdoz(&[("data/blocos.arrow", b"um"), ("assets/a.png", b"PNG")]), None, "Ana", "Primeira").unwrap();
        let v2 = gravar_com_historico(&fdoz(&[("data/blocos.arrow", b"dois")]), Some(&v1), "Ana", "Segunda").unwrap();
        let log = versoes(&v2).unwrap();
        assert_eq!(log.iter().map(|v| v.mensagem.as_str()).collect::<Vec<_>>(), ["Segunda", "Primeira"]);
        assert_eq!(log[0].autor, "Ana");
        let antiga = partes(&abrir_versao(&v2, &log[1].id).unwrap()).unwrap();
        assert_eq!(antiga, [("assets/a.png".to_string(), b"PNG".to_vec()), ("data/blocos.arrow".to_string(), b"um".to_vec())]);
        // O documento atual continua por fora do .git, para quem abre sem histórico.
        assert!(partes(&v2).unwrap().iter().any(|(n, b)| n == "data/blocos.arrow" && b == b"dois"));
    }

    #[test]
    fn salvar_sem_mudar_nada_nao_cria_versao_vazia() {
        let doc = fdoz(&[("data/blocos.arrow", b"igual")]);
        let v1 = gravar_com_historico(&doc, None, "", "Primeira").unwrap();
        let v2 = gravar_com_historico(&doc, Some(&v1), "", "De novo").unwrap();
        let log = versoes(&v2).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].autor, "FeatherDocs");
    }

    #[test]
    fn arquivo_sem_historico_tem_lista_vazia_e_lixo_da_erro() {
        assert!(versoes(&fdoz(&[("data/blocos.arrow", b"x")])).unwrap().is_empty());
        assert!(versoes(b"lixo").is_err());
        let v1 = gravar_com_historico(&fdoz(&[("data/blocos.arrow", b"x")]), None, "", "a").unwrap();
        assert!(abrir_versao(&v1, "0000000000000000000000000000000000000000").is_err());
        assert!(abrir_versao(&v1, "não é hash").is_err());
    }

    #[test]
    fn caminho_que_foge_da_pasta_e_recusado() {
        for nome in [".git/objects\\..\\..\\fora.txt", ".git/C:/fora.txt", ".git/../fora.txt", ".git//fora.txt"] {
            let ruim = fdoz(&[("data/blocos.arrow", b"x"), (nome, b"mal")]);
            assert!(versoes(&ruim).is_err(), "{nome}");
            assert!(gravar_com_historico(&fdoz(&[("data/blocos.arrow", b"y")]), Some(&ruim), "", "a").is_err(), "{nome}");
        }
    }

    #[test]
    fn bomba_de_zip_e_recusada() {
        let bomba = fdoz(&[("data/blocos.arrow", b"x"), (".git/config", &vec![0u8; (LIMITE_PARTE + 1) as usize])]);
        assert!(bomba.len() < 100_000, "comprime bem, como uma bomba");
        assert!(versoes(&bomba).is_err());
    }

    #[test]
    fn historico_grande_perde_as_versoes_mais_velhas() {
        // Dados que não comprimem (um gerador simples), para o histórico crescer de verdade.
        let mut semente = 1u64;
        let mut ruido = |n: usize| -> Vec<u8> {
            (0..n).map(|_| { semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (semente >> 56) as u8 }).collect()
        };
        let mut arquivo = gravar_com_historico(&fdoz(&[("data/blocos.arrow", &ruido(4096))]), None, "Ana", "v0").unwrap();
        for i in 1..60 {
            arquivo = gravar_com_historico(&fdoz(&[("data/blocos.arrow", &ruido(4096))]), Some(&arquivo), "Ana", &format!("v{i}")).unwrap();
        }
        let git: u64 = partes(&arquivo).unwrap().iter().filter(|(n, _)| n.starts_with(PASTA_GIT)).map(|(_, b)| b.len() as u64).sum();
        assert!(git <= LIMITE_HISTORICO, "{git}");
        let log = versoes(&arquivo).unwrap();
        assert!(log.len() < 60 && log.len() > 1, "{}", log.len());
        assert_eq!((log[0].mensagem.as_str(), log[0].autor.as_str()), ("v59", "Ana"));
        // A versão mais velha que ficou ainda abre, e a mais nova é o documento atual.
        assert!(abrir_versao(&arquivo, &log[log.len() - 1].id).is_ok());
        let atual = partes(&abrir_versao(&arquivo, &log[0].id).unwrap()).unwrap();
        assert!(partes(&arquivo).unwrap().contains(&atual[0]));
    }
}
