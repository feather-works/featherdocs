//! "Primeiros passos" (abre sozinho na primeira vez) e "O que há de novo" (na primeira abertura
//! depois de atualizar), como no FeatherSheet: lateral, cabeçalho com a marca, cartões, e
//! Anterior/Próximo no pé. Tudo embutido no app; nada é buscado na internet.
use crate::app::{App, Comando, JanelaAux, Msg};
use crate::localidade::{tr, trf};
use feather_ui::consulta::{self, Ilustracao};
use feather_ui::tema::*;
use pelicano::{button, column, row, text, Align, Element, Length, Space, Task};

/// Os passos, na ordem: título e ícone da lateral.
pub const PASSOS: [(&str, &str); 4] = [
    ("Boas-vindas", "rocket"),
    ("Aparência", "palette"),
    ("Formatos", "file-export"),
    ("Pronto", "circle-check"),
];

/// Uma versão publicada e o que ela trouxe.
pub struct Versao {
    pub numero: &'static str,
    pub data: &'static str,
    pub resumo: &'static str,
    pub ilustracao: Ilustracao,
    /// Cartões: ícone, título, descrição curta (até ~84 caracteres).
    pub destaques: &'static [(&'static str, &'static str, &'static str)],
    pub tambem: &'static [&'static str],
}

/// Da mais nova para a mais antiga. A primeira é a desta compilação.
pub const VERSOES: &[Versao] = &[
    Versao {
        numero: "0.1.2",
        data: "30/10/2026",
        resumo: "Paridade completa com o .docx e papel timbrado",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("file-export", ".docx completo", "Os 209 recursos do teste de paridade vão e voltam do Word."),
            ("file-plus", "Modelos novos", "17 modelos com formas e ilustrações, cinco deles de papel timbrado."),
            ("table", "Aba Tabela", "Aparece com o cursor na tabela: inserir, mesclar, bordas e fundo."),
            ("columns", "Colunas de texto", "Layout → Colunas divide a página em duas ou três colunas."),
        ],
        tambem: &[
            "Marca d'água predefinida (Rascunho, Confidencial…) na aba Design.",
            "Cabeçalho e rodapé diferentes na primeira página e nas páginas pares.",
            "Texto alternativo da imagem e mais propriedades do arquivo.",
            "Servidor MCP: assistentes de IA podem ler e editar documentos no seu computador.",
            "Arquivos malformados não derrubam mais o app; o tema Automático segue o GNOME.",
        ],
    },
    Versao {
        numero: "0.1.1",
        data: "28/10/2026",
        resumo: "Tabelas do Word e tela inicial",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("table", "Tabelas do Word completas", "Larguras, mescla horizontal e vertical, bordas, fundo e altura da linha."),
            ("layout-sidebar", "Tela inicial", "Modelos com a primeira página de verdade e os recentes com busca."),
            ("file-plus", "Seis modelos", "Carta, relatório, currículo, ata de reunião e trabalho acadêmico."),
            ("rocket", "Primeiros passos", "Este assistente abre na primeira vez; depois fica no menu Arquivos."),
        ],
        tambem: &[
            "A tabela do .docx volta ao Word igual: o LibreOffice desenha o arquivo regravado como o original.",
            "Inserir ou excluir linha e coluna leva junto o fundo, as larguras e a altura.",
            "Menu Arquivos no lugar da lista simples: Início, Recentes, Exportar e Informações.",
        ],
    },
    Versao {
        numero: "0.1.0",
        data: "28/10/2026",
        resumo: "Primeira versão pública",
        ilustracao: Ilustracao::Pronto,
        destaques: &[
            ("bolt", "Interface nativa no pelicano", "A mesma biblioteca do FeatherNotes e do FeatherSheet."),
            ("file-export", ".docx de ida e volta", "Parágrafos, estilos, listas, imagens, equações, notas e seções."),
            ("history", "Histórico dentro do .fdoz", "Cada vez que salva vira uma versão; nada é sobrescrito."),
            ("code", "LaTeX e Markdown", "Abre e exporta .tex (e projetos .zip) e .md."),
        ],
        tambem: &["Tudo roda no seu computador: sem conta, sem login e sem telemetria."],
    },
];

#[derive(Debug, Clone)]
pub enum Evento {
    AbrirPassos,
    AbrirNovidades,
    Passo(usize),
    Anterior,
    Proximo,
    /// Começar (último passo) ou Pular: fecha os Primeiros passos.
    Concluir,
    Versao(usize),
    VerNoGithub,
    FecharNovidades,
}

/// A versão desta compilação.
pub fn versao_atual() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

impl App {
    /// Na abertura: a primeira vez mostra os Primeiros passos; depois de uma atualização, as
    /// novidades. Nunca nos testes nem no modo de captura.
    pub(crate) fn janela_de_boas_vindas(&mut self) -> Option<JanelaAux> {
        if cfg!(test) || std::env::var_os("FEATHERDOCS_CAPTURAR").is_some() {
            return None;
        }
        self.boas_vindas_pendente()
    }

    /// A decisão de [`App::janela_de_boas_vindas`], sem as travas (testável).
    pub(crate) fn boas_vindas_pendente(&mut self) -> Option<JanelaAux> {
        let atual = versao_atual();
        // Quem já usava (havia preferências gravadas) está atualizando: vê as novidades.
        let instalacao_nova = !self.config.existia && !self.config.primeiros_passos_vistos;
        let janela = if instalacao_nova {
            Some(JanelaAux::PrimeirosPassos)
        } else if self.config.versao_vista != atual {
            self.novidades_versao = 0;
            Some(JanelaAux::Novidades)
        } else {
            None
        };
        if janela.is_some() {
            // Fechar pelo X também conta como visto.
            self.config.primeiros_passos_vistos = true;
            self.config.versao_vista = atual.to_owned();
            self.config.gravar();
        }
        janela
    }

    pub(crate) fn evento_boas_vindas(&mut self, e: Evento) -> Task<Msg> {
        let ultimo = PASSOS.len() - 1;
        match e {
            Evento::AbrirPassos => {
                self.menu_aberto = false;
                self.passo = 0;
                if !self.janelas.aberta(JanelaAux::PrimeirosPassos) {
                    return self.alternar_aux(JanelaAux::PrimeirosPassos);
                }
            }
            Evento::AbrirNovidades => {
                self.menu_aberto = false;
                self.novidades_versao = 0;
                if !self.janelas.aberta(JanelaAux::Novidades) {
                    return self.alternar_aux(JanelaAux::Novidades);
                }
            }
            Evento::Passo(i) => self.passo = i.min(ultimo),
            Evento::Anterior => self.passo = self.passo.saturating_sub(1),
            Evento::Proximo if self.passo >= ultimo => return self.evento_boas_vindas(Evento::Concluir),
            Evento::Proximo => self.passo += 1,
            Evento::Concluir => {
                self.passo = 0;
                if self.janelas.aberta(JanelaAux::PrimeirosPassos) {
                    return self.alternar_aux(JanelaAux::PrimeirosPassos);
                }
            }
            Evento::Versao(i) => self.novidades_versao = i.min(VERSOES.len() - 1),
            Evento::VerNoGithub => {
                let v = &VERSOES[self.novidades_versao.min(VERSOES.len() - 1)];
                if !cfg!(test) {
                    let _ = crate::app::abrir_no_sistema(std::ffi::OsStr::new(&format!("https://github.com/feather-works/featherdocs/releases/tag/v{}", v.numero)));
                }
            }
            Evento::FecharNovidades => {
                if self.janelas.aberta(JanelaAux::Novidades) {
                    return self.alternar_aux(JanelaAux::Novidades);
                }
            }
        }
        Task::none()
    }

    pub(crate) fn view_primeiros_passos(&self) -> Element<'_, Msg> {
        let passo = self.passo.min(PASSOS.len() - 1);
        let mut nav = column![consulta::rotulo_grupo(tr("Passos"))].spacing(2);
        for (i, (titulo, icone)) in PASSOS.iter().enumerate() {
            nav = nav.push(consulta::item_nav(format!("{}. {}", i + 1, tr(titulo)), icone, i == passo, Some(Msg::BoasVindas(Evento::Passo(i)))));
        }
        let pular = button(text(tr("Pular e configurar depois")).size(12))
            .padding([8, 12]).width(Length::FILL).style(consulta::estilo_cartao)
            .on_press(Msg::BoasVindas(Evento::Concluir));
        let ultimo = passo + 1 == PASSOS.len();
        let rodape = consulta::paginacao(
            (tr("Anterior"), (passo > 0).then_some(Msg::BoasVindas(Evento::Anterior))),
            (tr(if ultimo { "Começar" } else { "Próximo" }), Some(Msg::BoasVindas(Evento::Proximo))),
        );
        consulta::janela(consulta::lateral(nav, (!ultimo).then(|| pular.into())), self.conteudo_passo(passo), Some(rodape))
    }

    fn conteudo_passo(&self, passo: usize) -> Element<'_, Msg> {
        let opcao = |rotulo: &str, ativa: bool, m: Msg| {
            button(text(tr(rotulo).to_owned()).size(12.5)).padding([6, 13]).style(estilo_botao_pequeno(ativa)).on_press(m)
        };
        match passo {
            0 => column![
                consulta::cabecalho(tr("Boas-vindas ao FeatherDocs"),
                    tr("Documentos no seu computador, sem conta e sem telemetria. Em três passos curtos o app fica do seu jeito; tudo pode ser mudado depois em Configurações."),
                    Some(consulta::ilustracao(Ilustracao::Salvamento))),
                consulta::negrito(tr("O que você encontra aqui"), 18.),
                consulta::grade(vec![
                    consulta::cartao("file-export", tr(".docx de ida e volta"), tr("Abre e grava Word: estilos, listas, tabelas, imagens, notas e seções."), None),
                    consulta::cartao("lock", tr("Privado por padrão"), tr("Tudo roda no computador; nada dos seus documentos é enviado."), None),
                    consulta::cartao("history", tr("Histórico no arquivo"), tr("O .fdoz guarda cada versão salva; dá para voltar a qualquer uma."), None),
                    consulta::cartao("code", tr("LaTeX e Markdown"), tr("Abre e exporta .tex e .md, com o código ao lado da página."), None),
                    consulta::cartao("table", tr("Tabelas e gráficos"), tr("Tabelas do Word com mescla, bordas e fundo; gráficos da suíte."), None),
                    consulta::cartao("list", tr("Sumário e referências"), tr("Sumário, legendas, notas, bibliografia e índice gerados sozinhos."), None),
                ], 3),
            ].spacing(17).into(),
            1 => column![
                consulta::cabecalho(tr("Aparência"), tr("Idioma, tema e cor de destaque."), Some(consulta::ilustracao(Ilustracao::Aparencia))),
                feather_ui::configuracoes::linha(tr("Idioma"), tr("Idioma dos menus, da faixa e das janelas."),
                    crate::localidade::IDIOMAS.iter().fold(row![].spacing(4), |l, (codigo, nome)| {
                        l.push(opcao(nome, crate::localidade::atual() == *codigo, Msg::Idioma(codigo)))
                    })),
                feather_ui::configuracoes::linha(tr("Tema"), tr("Automático acompanha o sistema na abertura."), row![
                    opcao("Automático", self.config.tema_auto, Msg::TemaAuto),
                    opcao("Claro", !self.config.tema_auto && !self.config.escuro, Msg::Escuro(false)),
                    opcao("Escuro", !self.config.tema_auto && self.config.escuro, Msg::Escuro(true)),
                ].spacing(4)),
                feather_ui::configuracoes::linha(tr("Cor de destaque"), tr("Botões, seleção e a marca do app."),
                    ACENTOS.iter().enumerate().fold(row![].spacing(8), |l, (i, (nome, _, _))| l.push(opcao(nome, self.config.acento == i, Msg::Acento(i))))),
            ].spacing(17).into(),
            2 => column![
                consulta::cabecalho(tr("Formatos"), tr("O FeatherDocs trabalha em .fdoz e conversa com os outros formatos sem perder o seu arquivo."), Some(consulta::ilustracao(Ilustracao::Salvamento))),
                consulta::grade(vec![
                    consulta::cartao("device-floppy", tr(".fdoz — o arquivo do FeatherDocs"), tr("Guarda tudo, com o histórico de versões dentro."), None),
                    consulta::cartao("file-export", tr(".docx — Word"), tr("Abrir e salvar; o que não cabe no Word é avisado antes de gravar."), None),
                    consulta::cartao("code", tr(".tex e .zip — LaTeX"), tr("Abre o projeto (com as imagens) e exporta o código."), None),
                    consulta::cartao("file-code", tr(".md — Markdown"), tr("Para README, notas e sites."), None),
                    consulta::cartao("printer", tr("PDF"), tr("Exportar PDF (Ctrl+P) com as páginas como na tela."), None),
                ], 2),
                consulta::apagado(tr("Abrir um .docx não o transforma: ao salvar, o FeatherDocs pergunta onde gravar o .fdoz e o original fica como estava."), 11.5),
            ].spacing(17).into(),
            _ => column![
                consulta::cabecalho(tr("Tudo pronto"), tr("Algumas portas de entrada para quando precisar. Esta janela continua no menu Arquivos, em Primeiros passos."), Some(consulta::ilustracao(Ilustracao::Pronto))),
                consulta::grade(vec![
                    consulta::cartao("help", tr("Ajuda (F1)"), tr("Guia de uso e todos os atalhos."), Some(Msg::Comando(Comando::Ajuda))),
                    consulta::cartao("sparkles", tr("O que há de novo"), tr("As novidades de cada versão do FeatherDocs."), Some(Msg::BoasVindas(Evento::AbrirNovidades))),
                    consulta::cartao("folder-open", tr("Abrir um documento"), tr(".fdoz, .docx, .tex, .zip ou .md."), Some(Msg::Comando(Comando::Abrir))),
                    consulta::cartao("settings", tr("Configurações"), tr("Todas as preferências do app."), Some(Msg::Comando(Comando::Configuracoes))),
                ], 2),
            ].spacing(17).into(),
        }
    }

    pub(crate) fn view_novidades(&self) -> Element<'_, Msg> {
        let atual = self.novidades_versao.min(VERSOES.len() - 1);
        let mut nav = column![consulta::rotulo_grupo(tr("Versões"))].spacing(2);
        for (i, v) in VERSOES.iter().enumerate() {
            nav = nav.push(consulta::item_nav(trf("Versão {}", &[&v.numero]), if i == 0 { "sparkles" } else { "history" }, i == atual, Some(Msg::BoasVindas(Evento::Versao(i)))));
        }
        let v = &VERSOES[atual];
        let instalada = v.numero == versao_atual();
        let tambem = v.tambem.iter().fold(column![].spacing(8), |c, item| {
            c.push(row![feather_ui::icones::icone("check", 15.), text(tr(item).to_owned()).size(13).line_height(1.4)].spacing(9).align(Align::Start))
        });
        let conteudo = column![
            consulta::cabecalho(trf("Versão {}", &[&v.numero]),
                format!("{} · {}{}", v.data, tr(v.resumo), if instalada { tr(" · instalada neste computador") } else { "" }),
                Some(consulta::ilustracao(v.ilustracao))),
            consulta::negrito(tr("Destaques"), 18.),
            consulta::grade(v.destaques.iter().map(|(icone, titulo, descricao)| consulta::cartao(icone, tr(titulo), tr(descricao), None)).collect(), 2),
            consulta::divisor(),
            consulta::negrito(tr("Também nesta versão"), 18.),
            tambem,
        ].spacing(17);
        let rodape = row![
            Space::new(Length::FILL, 0.0),
            button(row![feather_ui::icones::icone("link", 15.), text(tr("Ver no GitHub")).size(12)].spacing(6).align(Align::Center))
                .padding([8, 13]).style(consulta::estilo_cartao).on_press(Msg::BoasVindas(Evento::VerNoGithub)),
            button(text("OK").size(12)).padding([8, 18]).style(estilo_botao_primario).on_press(Msg::BoasVindas(Evento::FecharNovidades)),
        ].spacing(10).align(Align::Center);
        consulta::janela(consulta::lateral(nav, None), conteudo.into(), Some(rodape.into()))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_versao_desta_compilacao_e_a_primeira_e_os_icones_existem() {
        assert_eq!(VERSOES[0].numero, versao_atual(), "acrescente as novidades da versão nova em VERSOES");
        for v in VERSOES {
            for (icone, _, descricao) in v.destaques {
                assert!(feather_ui::icones::existe(icone), "{icone}");
                assert!(descricao.chars().count() <= 90, "{descricao}");
            }
        }
        for (_, icone) in PASSOS {
            assert!(feather_ui::icones::existe(icone), "{icone}");
        }
    }
}
