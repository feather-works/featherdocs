//! A janela "Versões…": a lista de versões guardadas dentro do `.fdoz` (o histórico Git embutido),
//! com "Comparar com a atual" (as diferenças viram alterações sugeridas) e "Restaurar".
use super::{App, Msg};
use crate::compat::button;
use crate::localidade::{tr, trf};
use feather_ui::{icones::icone, tema::*};
use featherdocs_core::modelo::Documento;
use featherdocs_historico::Versao;
use pelicano::{column, container, horizontal_space, row, scrollable, text, Align, Element, Length, Task};
use std::path::PathBuf;

/// "02/11/2026 14:05" na hora de quem salvou.
pub(crate) fn quando(v: &Versao) -> String {
    let local = v.quando + i64::from(v.fuso);
    let (a, m, d) = crate::inicio::civil(local.div_euclid(86_400));
    let s = local.rem_euclid(86_400);
    format!("{d:02}/{m:02}/{a} {:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// O documento da versão `id` do arquivo `p`.
fn ler_versao(p: &std::path::Path, id: &str) -> Result<Documento, String> {
    let bytes = featherdocs_io::arquivo::ler(p).map_err(|e| e.to_string())?;
    featherdocs_io::fdoz::ler(&featherdocs_historico::abrir_versao(&bytes, id)?)
}

impl App {
    /// O `.fdoz` aberto, de onde vêm as versões.
    fn arquivo_com_versoes(&self) -> Option<PathBuf> {
        self.caminho.clone().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("fdoz")))
    }

    pub(super) fn versoes_abrir(&mut self) -> Task<Msg> {
        let Some(p) = self.arquivo_com_versoes() else {
            self.status = tr("Salve o documento como .fdoz: cada salvamento vira uma versão.").into();
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let bytes = featherdocs_io::arquivo::ler(&p).map_err(|e| e.to_string())?;
                    featherdocs_historico::versoes(&bytes)
                })
                .await
                .map_err(|e| e.to_string())?
            },
            Msg::VersoesLidas,
        )
    }

    pub(super) fn versoes_lidas(&mut self, r: Result<Vec<Versao>, String>) {
        match r {
            Ok(v) if v.is_empty() => self.status = tr("Este arquivo ainda não tem versões: elas começam no próximo salvamento.").into(),
            Ok(v) => self.versoes = Some(v),
            Err(e) => self.status = e,
        }
    }

    /// Abre a versão `id` para comparar com a atual ou, com `restaurar`, para ficar no lugar dela.
    pub(super) fn versao_pedir(&mut self, id: String, restaurar: bool) -> Task<Msg> {
        if restaurar && self.sessao.sujo() {
            self.status = tr("Salve o documento antes de restaurar uma versão: assim a de agora também fica no histórico.").into();
            return Task::none();
        }
        let Some(p) = self.arquivo_com_versoes() else { return Task::none() };
        let rotulo = self.versoes.as_ref().and_then(|v| v.iter().find(|x| x.id == id)).map_or_else(String::new, quando);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || ler_versao(&p, &id))
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|doc| (doc, rotulo, restaurar))
            },
            Msg::VersaoAberta,
        )
    }

    pub(super) fn versao_aberta(&mut self, r: Result<(Documento, String, bool), String>) {
        let (doc, rotulo, restaurar) = match r {
            Ok(x) => x,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.versoes = None;
        if restaurar {
            let Some(p) = self.arquivo_com_versoes() else { return };
            self.carregar(p, doc);
            // Fica como mudança não salva: salvar faz dela a versão atual, e a de antes continua no histórico.
            self.sessao.marcar_nao_gravado();
            self.status = trf("Versão de {} restaurada: salve para que ela vire a atual.", &[&rotulo]);
        } else {
            self.sessao.comparar_com(&doc, &trf("versão de {}", &[&rotulo]));
            self.painel_revisao = true;
            self.status = trf("Diferenças em relação à versão de {} marcadas como alterações", &[&rotulo]);
        }
    }

    pub(super) fn view_versoes<'a>(&'a self, versoes: &'a [Versao]) -> Element<'a, Msg> {
        let mut lista = column![].spacing(6);
        for (i, v) in versoes.iter().enumerate() {
            let rotulo = if i == 0 { format!("{} · {}", quando(v), tr("a atual")) } else { quando(v) };
            let mut linha = row![
                icone("history", 16.),
                column![text(rotulo).size(14), text(format!("{} · {}", v.mensagem, v.autor)).size(12)].spacing(2),
                horizontal_space(),
            ]
            .spacing(10)
            .align(Align::Center);
            if i > 0 {
                linha = linha
                    .push(button(text(tr("Comparar com a atual"))).on_press(Msg::VersaoComparar(v.id.clone())).style(estilo_botao_secundario))
                    .push(button(text(tr("Restaurar"))).on_press(Msg::VersaoRestaurar(v.id.clone())).style(estilo_botao_secundario));
            }
            lista = lista.push(linha);
        }
        container(
            column![
                row![
                    text(tr("Versões deste arquivo")).size(19),
                    horizontal_space(),
                    button(text(tr("Fechar"))).on_press(Msg::FecharVersoes).style(estilo_botao_secundario),
                ]
                .align(Align::Center),
                text(tr("Cada salvamento guarda uma versão dentro do próprio .fdoz. Restaurar não apaga nada: a versão de agora continua na lista.")).size(13),
                scrollable(lista).height(Length::FILL),
            ]
            .spacing(15),
        )
        .max_width(760.)
        .padding(30u16)
        .width(Length::FILL)
        .height(Length::FILL)
        .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_da_versao_na_hora_de_quem_salvou() {
        // 02/11/2026 17:05 UTC, salvo em Brasília (−3 h).
        let v = Versao { id: String::new(), quando: 1_793_639_100, fuso: -3 * 3600, autor: String::new(), mensagem: String::new() };
        assert_eq!(quando(&v), "02/11/2026 14:05");
    }

    #[test]
    fn restaurar_e_comparar_uma_versao_do_arquivo() {
        let p = std::env::temp_dir().join(format!("featherdocs-versoes-{}.fdoz", std::process::id()));
        let doc = |t: &str| featherdocs_io::markdown::ler(t);
        let primeira = crate::arquivos::com_historico(featherdocs_io::fdoz::gravar(&doc("Primeiro texto.")).unwrap(), None).unwrap();
        std::fs::write(&p, &primeira).unwrap();
        let segunda = crate::arquivos::com_historico(featherdocs_io::fdoz::gravar(&doc("Segundo texto.")).unwrap(), Some(&p)).unwrap();
        std::fs::write(&p, &segunda).unwrap();
        let versoes = featherdocs_historico::versoes(&segunda).unwrap();
        assert_eq!(versoes.len(), 2);

        let mut app = App::default();
        app.caminho = Some(p.clone());
        app.versoes_lidas(Ok(versoes.clone()));
        let antiga = ler_versao(&p, &versoes[1].id).unwrap();
        app.versao_aberta(Ok((antiga.clone(), quando(&versoes[1]), false)));
        assert!(app.versoes.is_none() && app.painel_revisao, "comparar abre o painel de revisão");

        app.versao_aberta(Ok((antiga, quando(&versoes[1]), true)));
        assert!(featherdocs_core::texto::plano(match &app.sessao.documento().blocos[0].conteudo {
            featherdocs_core::modelo::Conteudo::Paragrafo { texto } => texto,
            _ => panic!("parágrafo"),
        })
        .contains("Primeiro"));
        assert!(app.sessao.sujo(), "restaurada, espera o salvamento");
        assert_eq!(app.caminho.as_deref(), Some(p.as_path()));
        let _ = std::fs::remove_file(p);
    }
}
