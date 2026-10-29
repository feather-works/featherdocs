//! Quem guarda o documento que as ferramentas mexem: um arquivo aberto pelo próprio servidor
//! ([`Avulso`]) ou, depois, o aplicativo aberto na tela. As ferramentas só falam com este trait.
use std::path::{Path, PathBuf};

use featherdocs_core::sessao::Sessao;
use featherdocs_io::Formato;

pub trait Hospede {
    fn sessao(&self) -> &Sessao;
    fn sessao_mut(&mut self) -> &mut Sessao;
    /// Arquivo de onde o documento veio, se veio de um.
    fn caminho(&self) -> Option<PathBuf>;
    fn abrir(&mut self, caminho: &Path) -> Result<String, String>;
    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String>;
    /// Troca o documento inteiro (modelo novo, documento em branco).
    fn novo(&mut self, sessao: Sessao);
    fn ao_vivo(&self) -> bool {
        false
    }
}

/// Sem o aplicativo: o documento na memória do servidor. Gravar é sempre pedido explícito.
#[derive(Default)]
pub struct Avulso {
    pub sessao: Sessao,
    pub caminho: Option<PathBuf>,
}

fn extensao(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// Lê um documento do disco em qualquer formato que o FeatherDocs abre.
pub fn ler(caminho: &Path) -> Result<Sessao, String> {
    let formato = Formato::da_extensao(&extensao(caminho)).ok_or("Formato não suportado: use .fdoz, .docx, .md, .tex ou .zip.")?;
    let bytes = std::fs::read(caminho).map_err(|e| format!("Não consegui ler {}: {e}", caminho.display()))?;
    let doc = featherdocs_io::abrir_na_pasta(formato, &bytes, caminho.parent())?;
    Ok(Sessao::com_documento(doc))
}

/// Grava pelo formato da extensão, num `.parcial` renomeado por cima (uma queda no meio não estraga o arquivo).
pub fn gravar(sessao: &Sessao, caminho: &Path) -> Result<(), String> {
    let doc = sessao.documento();
    let bytes = match extensao(caminho).as_str() {
        "fdoz" => featherdocs_io::fdoz::gravar(doc)?,
        "docx" => featherdocs_io::docx::gravar(doc)?,
        "md" | "markdown" => featherdocs_io::exportar::markdown(doc).into_bytes(),
        "tex" => featherdocs_io::exportar::latex(doc).into_bytes(),
        "zip" => featherdocs_io::exportar::latex_zip(doc)?,
        _ => return Err("Formato de gravação não suportado: use .fdoz, .docx, .md, .tex ou .zip.".into()),
    };
    let parcial = caminho.with_extension(format!("{}.parcial", extensao(caminho)));
    std::fs::write(&parcial, bytes).map_err(|e| format!("Não consegui gravar {}: {e}", parcial.display()))?;
    std::fs::rename(&parcial, caminho).map_err(|e| format!("Não consegui gravar {}: {e}", caminho.display()))
}

impl Hospede for Avulso {
    fn sessao(&self) -> &Sessao {
        &self.sessao
    }
    fn sessao_mut(&mut self) -> &mut Sessao {
        &mut self.sessao
    }
    fn caminho(&self) -> Option<PathBuf> {
        self.caminho.clone()
    }
    fn abrir(&mut self, caminho: &Path) -> Result<String, String> {
        self.sessao = ler(caminho)?;
        self.caminho = Some(caminho.to_path_buf());
        Ok(format!("Aberto {} ({} página(s)).", caminho.display(), self.sessao.total_de_paginas()))
    }
    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String> {
        let destino = caminho.map(Path::to_path_buf).or_else(|| self.caminho.clone()).ok_or("O documento ainda não tem arquivo: informe `caminho`.")?;
        gravar(&self.sessao, &destino)?;
        self.sessao.marcar_gravado();
        if extensao(&destino) == "fdoz" {
            self.caminho = Some(destino.clone());
        }
        Ok(format!("Salvo em {}.", destino.display()))
    }
    fn novo(&mut self, sessao: Sessao) {
        self.sessao = sessao;
        self.caminho = None;
    }
}
