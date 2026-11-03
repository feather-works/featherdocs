//! A ponte com a IA (o `featherdocs-mcp`): ligada, as ferramentas `docs_*` agem no documento da
//! tela. Cada edição é um passo de Ctrl+Z, como as da pessoa; ver `featherdocs_mcp::ponte`.
use super::{arquivos, App, Msg};
use crate::localidade::{tr, trf};
use featherdocs_core::sessao::Sessao;
use featherdocs_mcp::hospede::Hospede;
use pelicano::Task;
use std::path::{Path, PathBuf};

impl Hospede for App {
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
        let doc = arquivos::ler(caminho)?;
        self.carregar(caminho.to_path_buf(), doc);
        Ok(format!("Aberto {} no FeatherDocs ({} página(s)).", caminho.display(), self.sessao.total_de_paginas()))
    }
    /// Salva já (a IA espera a resposta): o mesmo caminho do Ctrl+S, com o histórico no `.fdoz`.
    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String> {
        let destino = caminho.map(Path::to_path_buf).or_else(|| self.caminho.clone()).ok_or("O documento ainda não tem arquivo: informe `caminho`.")?;
        let ext = destino.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let bytes = arquivos::bytes(self.sessao.documento(), &ext)?;
        let bytes = if ext == "fdoz" { arquivos::com_historico(bytes, self.caminho.as_deref())? } else { bytes };
        arquivos::gravar(&destino, &bytes)?;
        if ext == "fdoz" {
            self.config.lembrar(&destino);
            self.caminho = Some(destino.clone());
            self.sessao.marcar_gravado();
        }
        Ok(format!("Salvo em {}.", destino.display()))
    }
    fn novo(&mut self, sessao: Sessao) {
        self.sessao = sessao;
        self.quadros.clear();
        self.caminho = None;
    }
}

impl App {
    /// Liga ou desliga a ponte (e guarda a escolha).
    pub(super) fn ponte_alternar(&mut self) {
        self.config.ponte_ia = !self.config.ponte_ia;
        self.config.gravar();
        self.ponte_aplicar();
    }

    /// Abre ou fecha a porta conforme a configuração.
    pub(super) fn ponte_aplicar(&mut self) {
        if !self.config.ponte_ia {
            self.ponte = None;
            return;
        }
        if self.ponte.is_some() {
            return;
        }
        match featherdocs_mcp::ponte::arquivo().ok_or_else(|| "sem pasta de dados".to_string()).and_then(|a| featherdocs_mcp::ponte::Servidor::iniciar(a).map_err(|e| e.to_string())) {
            Ok(s) => {
                self.ponte = Some(s);
                self.status = tr("Assistente de IA ligado: as ferramentas do featherdocs-mcp agem neste documento.").into();
            }
            Err(e) => {
                self.config.ponte_ia = false;
                self.status = trf("Não consegui ligar o assistente de IA: {}", &[&e]);
            }
        }
    }

    /// Atende o que a IA pediu desde o último pulso, na ordem, e redesenha.
    pub(super) fn ponte_atender(&mut self) -> Task<Msg> {
        let pedidos = self.ponte.as_ref().map(featherdocs_mcp::ponte::Servidor::pedidos).unwrap_or_default();
        if pedidos.is_empty() {
            return Task::none();
        }
        for p in pedidos {
            let resultado = featherdocs_mcp::chamar_ferramenta(self, &p.params);
            p.responder(resultado);
        }
        self.atualizar();
        Task::none()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_ia_edita_o_documento_da_tela_e_o_ctrl_z_desfaz() {
        let mut app = App::default();
        let r = featherdocs_mcp::chamar_ferramenta(&mut app, &serde_json::json!({ "name": "docs_trocar", "arguments": { "de": 0, "markdown": "# Plano da IA\\n\\nTexto novo." } }));
        assert_eq!(r["isError"], false, "{r}");
        assert!(featherdocs_core::texto::plano(app.sessao.documento().blocos[0].conteudo.textos()[0]).contains("Plano da IA"));
        assert!(app.sessao.sujo(), "a mudança aparece como não salva");
        app.sessao.desfazer();
        assert!(!featherdocs_core::texto::plano(app.sessao.documento().blocos[0].conteudo.textos()[0]).contains("Plano da IA"));
    }
}
