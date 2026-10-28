//! Estado e eventos da aplicação. Regras de edição ficam no featherdocs-core.
use crate::{
    arquivos,
    ajuda,
    configuracao::Configuracao,
    cor_ui::AlvoCor,
    faixa::{self, Aba},
    localidade::{tr, trf},
    pagina::Pagina,
    painel_codigo::{self, PainelCodigo},
    quadro::{Operacao, Quadro},
};
use featherdocs_io::dividido::Fonte;
use feather_ui::{icones::icone, tema::*, janelas::Janelas};
use featherdocs_core::{busca::Opcoes, geometria::Visao, modelo::Documento, sessao::Sessao};
use crate::compat::{button, canvas, keyboard::{self, Modificadores as _}, window, Vector};
use pelicano::{
    column, container, horizontal_space, row, scrollable, text, text_input, toggler, Align, Border, Color, Element, Event,
    Length, Padding, Point, Shadow, Size, Space, Status, Style, Subscription, Task, WindowCmd, WindowId,
};
use std::{collections::HashMap, path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JanelaAux { Configuracoes, Ajuda, Monitor, PrimeirosPassos, Novidades }


#[derive(Clone, Debug)]
pub enum Comando {
    Novo,
    Abrir,
    Salvar,
    SalvarComo,
    Exportar,
    ExportarPdf,
    Desfazer,
    Refazer,
    Copiar,
    Recortar,
    Colar,
    Pincel,
    ColarSemFormatacao,
    Formato(u8),
    /// Sobrescrito (1) ou subscrito (-1); repetir desliga.
    Posicao(i8),
    TamanhoPasso(i32),
    LimparFormatacao,
    Maiusculas(&'static str),
    Recuar(i32),
    Lista(&'static str),
    ListaReiniciar,
    Entrelinha(f32),
    /// Espaço antes (`true`) ou depois do parágrafo, em pontos; o outro lado fica como está.
    Espaco(bool, f32),
    Borda(u8),
    /// Número de página: modelo e lugar (`base_centro`, `topo_direita`…).
    NumeroPagina(&'static str, &'static str),
    NumeroRemover,
    Legenda(&'static str),
    IndiceInserir,
    /// Lista de figuras (1) ou de tabelas (2).
    ListaDe(u8),
    SumarioRemover,
    SumarioNiveis(u8),
    ReferenciasAtualizar,
    /// Margens em cm: esquerda, direita, superior, inferior.
    Margens([f32; 4]),
    NumerarLinhas(&'static str),
    PrimeiraDiferente,
    Vertical(&'static str),
    /// Marca d'água com este texto, na cor padrão; vazio tira.
    MarcaDagua(&'static str),
    Proteger,
    Formulario(crate::dialogos::Tipo),
    ImportarBib,
    Bibliografia(bool),
    EstiloCitacao(&'static str),
    Estilo(&'static str),
    EstiloNome(u32),
    /// Atualiza o estilo com nome do cursor para a formatação atual dele.
    EstiloAtualizar,
    EstiloExcluir,
    Alinhar(&'static str),
    Tabela,
    TabelaTam(usize, usize),
    Grafico(featherdocs_io::grafico_docs::Tipo),
    AtualizarGrafico,
    TabelaEstrutura(&'static str),
    TabelaMesclar,
    TabelaDesmesclar,
    TabelaCabecalho,
    TabelaExcluir,
    Imagem,
    Quebra,
    /// Quebra de linha dentro do parágrafo (Layout → Quebras → Disposição do texto; Shift+Enter).
    QuebraDeLinha,
    /// Um dos estilos rápidos (`dialogos::RAPIDOS`).
    EstiloRapido(usize),
    Sumario,
    Papel(&'static str),
    Orientacao(bool),
    Modo(&'static str),
    /// Aceitar (`true`) ou rejeitar; o segundo diz se é o documento todo ou só a seleção/cursor.
    Revisoes(bool, bool),
    PainelRevisao,
    Comparar,
    Estrutura,
    Marcas,
    /// Liga o painel de código nesta língua; de novo na mesma, desliga.
    Codigo(Fonte),
    Zoom(f32),
    Configuracoes,
    Buscar,
    Menu,
    Ajuda,
    Monitor,
    /// Um campo de Layout → Parágrafo mudou (`faixa::aplicar_campo`): o campo e o texto digitado.
    CampoParagrafo(u8, String),
}
#[derive(Clone, Debug)]
pub enum Destino {
    Novo,
    Abrir,
    /// Um arquivo já escolhido: recentes ou arrastado para a janela.
    AbrirCaminho(PathBuf),
    /// Um documento novo a partir de um modelo da tela inicial (`featherdocs_core::modelos`).
    Modelo(&'static str),
    Fechar(window::Id),
}
#[derive(Clone, Debug)]
pub enum Msg {
    Inicio(crate::inicio::Evento),
    /// A borda do painel Estrutura: apertar, mover (x na janela) e soltar.
    EstruturaArrastar,
    EstruturaMover(f32),
    EstruturaSoltar,
    BoasVindas(crate::boas_vindas::Evento),
    Atalho(Comando),
    Comando(Comando),
    Aba(Aba),
    Fonte(String),
    Tamanho(u16),
    /// O seletor de cor nativo (ou um menu) escolheu a cor do alvo (`#rrggbb`; vazio = sem cor).
    CorEscolhida(AlvoCor, String),
    AjudaEvento(feather_ui::ajuda::Evento),
    Tecla(keyboard::Key, keyboard::Modifiers, Option<String>),
    Clicar(usize, f32, f32, bool, u8),
    /// O botão do mouse foi solto: acabou o arrasto da seleção.
    Soltar,
    /// Evento da janela do Monitor (F12).
    MonitorEvento(feather_ui::monitor::Evento),
    /// Uma amostra de memória e CPU por segundo, enquanto o Monitor está aberto.
    MonitorPulso,
    /// Abre (ou fecha, com `None`) um menu que desce da faixa.
    MenuFaixa(Option<crate::faixa::MenuFaixa>),
    /// Onde o ponteiro está sobre a faixa: o menu aberto desce dali.
    PonteiroFaixa(Point),
    IrBloco(u64),
    Colado(Option<String>),
    ColadoPuro(Option<String>),
    FormAbrir(crate::dialogos::Tipo),
    AbrirRecente(PathBuf),
    BibLido(Result<Option<String>, String>),
    /// Botão direito na página: página, x, y no papel e o ponto do mouse na janela.
    BotaoDireito(usize, f32, f32, Point),
    CtrlClique(usize, f32, f32),
    AbrirLink,
    /// Ação escolhida no menu do botão direito (fecha o menu e executa).
    DoMenu(Box<Msg>),
    FecharContexto,
    Rolou(Vector),
    Comparado(Result<Option<(PathBuf, Documento)>, String>),
    ComentarioIr(u64),
    ComentarioResolver(u64, bool),
    ComentarioExcluir(u64),
    FormCampo(usize, String),
    FormOk,
    FormFechar,
    FormRemoverLink,
    /// Referência cruzada: o alvo escolhido.
    FormEscolher(u64),
    Aberto(Result<Option<(PathBuf, Documento)>, String>),
    Gravado(Result<Option<(PathBuf, u64, bool)>, String>),
    /// O formato escolhido para exportar não leva tudo: o caminho e o que se perde (pede confirmação).
    PerdasAoExportar(PathBuf, Vec<&'static str>),
    ExportarMesmoAssim,
    FecharPerdas,
    /// A janela Exportar PDF com as opções mudadas.
    OpcoesPdf(crate::pdf::OpcoesPdf),
    ConfirmarPdf,
    Imagem(Result<Option<(String, f32, f32)>, String>),
    AbrirEfeitosImagem,
    AplicarEfeitoImagem(&'static str),
    FecharEfeitosImagem,
    Fechar(window::Id),
    FecharAuxiliar(JanelaAux),
    /// A janela principal mudou de tamanho (a foto do modo captura sai nesse tamanho).
    JanelaRedimensionada(Size),
    ArrastarJanela(window::Id),
    Minimizar(window::Id),
    AlternarMaximizar(window::Id),
    AtalhoJanela(window::Id, keyboard::Key, keyboard::Modifiers),
    BarraPropria(bool),
    /// (0 = LaTeX, 1 = Markdown, valor)
    NumerosDeLinha(usize, bool),
    TemaCodigo(usize, usize),
    /// O painel lateral: 0 estrutura, 1 arquivos do projeto, 2 o que não foi convertido.
    Lateral(u8),
    /// Leva o código LaTeX (e a página) à linha `n` (1…).
    IrLinhaCodigo(usize),
    /// Clique num arquivo da lista do projeto.
    AbrirArquivo(String),
    FecharArquivo,
    ZoomArquivo(f32),
    Descartar,
    SalvarEContinuar,
    Cancelar,
    Escuro(bool),
    TemaAuto,
    Acento(usize),
    /// Troca o idioma da interface (`pt-BR`, `en-US`, `es-ES`).
    Idioma(&'static str),
    LogoPadrao,
    ConfigSecao(u8),
    AlternarFaixa,
    Consulta(String),
    Substituicao(String),
    /// Liga/desliga uma opção do Localizar: 0 maiúsculas, 1 palavra inteira, 2 expressão regular.
    BuscaOpcao(u8),
    BuscaIr(i32),
    AlternarSubstituir,
    Substituir(bool),
    /// O texto do editor do painel de código mudou (o `on_change` do pelicano).
    CodigoMudou,
    /// Clique no editor do painel de código: o foco sai da página.
    CodigoFoco,
    /// Uma tecla ou um clique terminou: o cursor do código pode ter ido a outro bloco.
    CodigoCursor,
    /// Pulso enquanto há código digitado à espera da pausa.
    CodigoPulso,
    CodigoFechar,
    /// Modo de captura (`FEATHERDOCS_CAPTURAR=arquivo.png`): prepara e fotografa.
    /// Rola a galeria de estilos da faixa (± blocos).
    GaleriaEstilos(i32),
    CapturarTela,
    /// Um botão da aba Desenhar (`tinta_docs`).
    Desenhar(feather_ui::tinta::Evento),
    /// A camada de tinta da página capturou um traço, uma apagada ou um laço.
    Tinta(usize, feather_ui::tinta::EventoCamada),
}
pub struct App {
    pub sessao: Sessao,
    pub config: Configuracao,
    pub aba: Aba,
    pub visao: Visao,
    quadros: Vec<Quadro>,
    pub(crate) imagens: HashMap<String, pelicano::Image>,
    pub(crate) caminho: Option<PathBuf>,
    pub(crate) status: String,
    foco_documento: bool,
    ocupado: bool,
    pagina_ativa: usize,
    pub(crate) janelas: Janelas<JanelaAux>,
    ultimo_clique_barra: Option<Instant>,
    busca: bool,
    consulta: String,
    substituicao: String,
    pendente: Option<Destino>,
    /// Exportação à espera de confirmação: o arquivo escolhido e o que o formato não leva.
    /// O aviso do que fica de fora: ao exportar para o arquivo, ou (sem arquivo) ao ver o código em Markdown.
    perdas_exportar: Option<(Option<PathBuf>, Vec<&'static str>)>,
    continuar_apos_salvar: bool,
    pub(crate) menu_aberto: bool,
    /// Bloco alvo, bytes originais e preset durante a edição de imagem.
    pub(crate) efeito_imagem: Option<(u64, String, &'static str)>,
    config_secao: u8,
    ajuda_estado: feather_ui::ajuda::Estado,
    /// O painel de código LaTeX/Markdown (modo dividido), quando aberto.
    codigo: Option<PainelCodigo>,
    /// O texto que a pessoa escreveu (o `.tex`/`.md` aberto ou o último código aplicado): o
    /// painel o mostra na forma dela enquanto ele ainda descrever o documento.
    fonte_original: Option<(Fonte, String)>,
    /// Os arquivos do projeto LaTeX aberto (.zip ou a pasta do .tex), para o painel Estrutura.
    arquivos_projeto: Vec<String>,
    /// A lista do painel lateral (ver `Msg::Lateral`).
    lateral: u8,
    /// O que a leitura do LaTeX não converteu: na abertura e a cada código aplicado.
    avisos_latex: Vec<featherdocs_io::latex::Aviso>,
    /// O projeto LaTeX aberto (.zip ou .tex), de onde se leem os arquivos da lista.
    projeto_caminho: Option<PathBuf>,
    /// Um arquivo do projeto aberto no lugar do código: o nome e o que mostrar.
    arquivo_visto: Option<(String, Visto)>,
    zoom_arquivo: f32,
    passo_captura: u8,
    /// O diálogo de formulário aberto (link, nota, cabeçalho, margens…).
    formulario: Option<crate::dialogos::Formulario>,
    /// A janela Exportar PDF aberta, com o que a pessoa já escolheu.
    opcoes_pdf: Option<crate::pdf::OpcoesPdf>,
    /// Painel lateral de revisão (estatísticas, comentários, alterações).
    painel_revisao: bool,
    /// O primeiro bloco visível da galeria de estilos da faixa.
    pub galeria_estilos: usize,
    /// Arrastando a seleção com o mouse: a página onde o mouse entra continua a seleção.
    arrastando: bool,
    /// O menu aberto por baixo da faixa e o ponto da faixa de onde ele desce.
    pub menu_faixa: Option<crate::faixa::MenuFaixa>,
    ponteiro_faixa: Point,
    ponto_menu_faixa: Point,
    /// O texto que a pessoa está digitando num campo de Layout → Parágrafo (`faixa::campo`).
    pub(crate) campo_faixa: Option<(u8, String)>,
    /// A aba Desenhar: ferramentas e o que está sendo apagado/escolhido.
    pub(crate) tinta: crate::tinta_docs::EstadoTinta,
    monitor: feather_ui::monitor::Monitor,
    /// Menu do botão direito aberto, no ponto da janela.
    menu_contexto: Option<Point>,
    /// Quanto a página está rolada (para refazer só as páginas perto da vista).
    rolagem: Vector,
    /// A versão do documento de que cada quadro saiu: o que ficou longe da vista é refeito ao rolar até ele.
    versao_dos_quadros: Vec<u64>,
    busca_opcoes: Opcoes,
    /// "3 de 12" (ou o erro da expressão) da última busca, mostrado no painel.
    busca_contagem: String,
    /// A linha de Substituir aberta no painel de busca.
    substituir_visivel: bool,
    /// A nota sob o botão direito, para o item "Editar nota" do menu.
    nota_contexto: Option<crate::dialogos::Tipo>,
    /// O último título mandado a cada janela (o `title` do daemon do Iced vira `WindowCmd::Title`).
    titulos: HashMap<WindowId, String>,
    /// O tamanho da janela principal (a foto do modo captura, a largura da tela inicial).
    pub(crate) tamanho_principal: Size,
    /// Tela inicial (abertura sem arquivo) e o backstage do menu Arquivos.
    pub(crate) inicio: crate::inicio::Inicio,
    /// O passo aberto nos Primeiros passos e a versão aberta em O que há de novo.
    pub(crate) passo: usize,
    pub(crate) novidades_versao: usize,
    /// A borda do painel Estrutura está sendo arrastada.
    arrastando_estrutura: bool,
    /// O evento que chega ao `on_window_event` não foi usado por widget nenhum.
    evento_livre: std::cell::Cell<bool>,
}
impl Default for App {
    fn default() -> Self {
        let sessao = Sessao::default();
        let visao = sessao.visao(0, 0);
        let mut app = Self {
            sessao,
            visao,
            config: Configuracao::default(),
            aba: Aba::Inicio,
            quadros: vec![],
            imagens: HashMap::new(),
            caminho: None,
            status: String::new(),
            foco_documento: true,
            ocupado: false,
            pagina_ativa: 0,
            janelas: Janelas::default(),
            ultimo_clique_barra: None,
            busca: false,
            consulta: String::new(),
            substituicao: String::new(),
            pendente: None,
            perdas_exportar: None,
            continuar_apos_salvar: false,
            menu_aberto: false,
            efeito_imagem: None,
            config_secao: 0,
            ajuda_estado: Default::default(),
            codigo: None,
            fonte_original: None,
            arquivos_projeto: Vec::new(),
            lateral: 0,
            avisos_latex: Vec::new(),
            projeto_caminho: None,
            arquivo_visto: None,
            zoom_arquivo: 1.,
            passo_captura: 0,
            formulario: None,
            opcoes_pdf: None,
            painel_revisao: false,
            galeria_estilos: 0,
            arrastando: false,
            menu_faixa: None,
            ponteiro_faixa: Point::ORIGIN,
            ponto_menu_faixa: Point::ORIGIN,
            campo_faixa: None,
            tinta: Default::default(),
            monitor: feather_ui::monitor::Monitor::novo("FeatherDocs"),
            menu_contexto: None,
            rolagem: Vector::default(),
            versao_dos_quadros: Vec::new(),
            busca_opcoes: Opcoes::default(),
            busca_contagem: String::new(),
            substituir_visivel: false,
            nota_contexto: None,
            titulos: HashMap::new(),
            tamanho_principal: Size::new(1200.0, 800.0),
            evento_livre: std::cell::Cell::new(false),
            inicio: crate::inicio::Inicio::default(),
            passo: 0,
            novidades_versao: 0,
            arrastando_estrutura: false,
        };
        app.atualizar();
        app
    }
}
impl App {
    pub fn novo() -> (Self, Task<Msg>) {
        let mut app = Self {
            config: Configuracao::carregar(),
            ..Self::default()
        };
        crate::localidade::definir(&app.config.idioma);
        definir_tema(tema_escuro(app.config.tema_auto, app.config.escuro), app.config.acento);
        feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherDocs,
            app.config.logo_usa_acento());
        // Nunca fazer parse DOCX/FDOZ antes de criar a janela.
        let inicial = std::env::args_os().nth(1).map(PathBuf::from);
        if inicial.is_some() {
            app.ocupado = true;
            app.status = tr("Abrindo documento em segundo plano…").into();
        }
        // A principal nasce escondida (`start_hidden` no main) e abre aqui, com a moldura da configuração.
        let id = WindowId::MAIN;
        let janela = Task::open_window(id, feather_ui::marca::preparar_janela(pelicano::Settings {
            title: app.titulo(id),
            size: Size::new(1200.0, 800.0),
            decorations: !app.config.barra_propria,
            close_on_request: false,
            ..Default::default()
        }));
        app.janelas.principal = Some(id);
        // Sem arquivo na linha de comando, abre na tela inicial (modelos e recentes), como o FeatherSheet.
        app.inicio.tela_cheia = inicial.is_none() && std::env::var_os("FEATHERDOCS_CAPTURAR").is_none();
        let janela = match app.janela_de_boas_vindas() {
            Some(aux) => Task::batch([janela, app.alternar_aux(aux)]),
            None => janela,
        };
        let tarefas = if let Some(p) = inicial {
            Task::batch([
                janela,
                Task::perform(
                    async move { Self::carregar_em_background(p).await.map(Some) },
                    Msg::Aberto,
                ),
            ])
        } else {
            janela
        };
        (app, tarefas)
    }

    /// Carregamento fora da thread da interface, inclusive abertura via CLI.
    async fn carregar_em_background(p: PathBuf) -> Result<(PathBuf, Documento), String> {
        tokio::task::spawn_blocking(move || arquivos::ler(&p).map(|doc| (p, doc)))
            .await
            .map_err(|e| trf("Falha ao processar documento: {}", &[&e]))?
    }
    pub(crate) fn alternar_aux(&mut self, tipo: JanelaAux) -> Task<Msg> {
        let tamanho = match tipo {
            JanelaAux::Configuracoes => Size::new(900.0, 640.0),
            JanelaAux::Ajuda => Size::new(980.0, 700.0),
            JanelaAux::Monitor => Size::new(980.0, 640.0),
            JanelaAux::PrimeirosPassos | JanelaAux::Novidades => Size::new(1000.0, 680.0),
        };
        self.janelas.alternar(tipo, tamanho, self.config.barra_propria)
    }
    fn acoes_janela(id: window::Id) -> feather_ui::cromo::AcoesJanela<Msg> {
        feather_ui::cromo::AcoesJanela {
            arrastar: Msg::ArrastarJanela(id),
            minimizar: Msg::Minimizar(id),
            maximizar: Msg::AlternarMaximizar(id),
            fechar: Msg::Fechar(id),
        }
    }
    pub fn painel_revisao(&self) -> bool {
        self.painel_revisao
    }
    pub fn titulo(&self, janela: window::Id) -> String {
        match self.janelas.tipo(janela) {
            Some(JanelaAux::Configuracoes) => return tr("Configurações do FeatherDocs").into(),
            Some(JanelaAux::Ajuda) => return tr("Ajuda do FeatherDocs").into(),
            Some(JanelaAux::Monitor) => return tr("Monitor do FeatherDocs").into(),
            Some(JanelaAux::PrimeirosPassos) => return tr("Primeiros passos").into(),
            Some(JanelaAux::Novidades) => return tr("O que há de novo no FeatherDocs").into(),
            None => {}
        }
        if self.inicio.tela_cheia {
            return "FeatherDocs".into();
        }
        let nome = self
            .caminho
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("sem_titulo").into());
        format!(
            "{}{} — FeatherDocs",
            if self.sessao.estado().sujo { "* " } else { "" },
            nome
        )
    }
    pub fn tema(&self, _janela: window::Id) -> pelicano::Theme {
        tema()
    }
    /// Só o cursor e a seleção mudaram: refaz a visão, sem redesenhar as páginas (o arrasto do mouse
    /// manda um evento por pixel; refazer todas as páginas a cada um travava a seleção).
    fn atualizar_cursor(&mut self) {
        let total = self.sessao.total_de_paginas();
        self.visao = self.sessao.visao(0, total.saturating_sub(1));
        self.pagina_ativa = self.visao.cursor.map_or(self.pagina_ativa, |c| c.pagina).min(total.saturating_sub(1));
        self.sincronizar_codigo();
    }
    /// As páginas como saem no papel (sem grade nem marcas de quebra), para o PDF.
    fn quadros_impressos(&self) -> Vec<Quadro> {
        // Uma página que falhe fica como na tela, para os links não apontarem para a página errada.
        (0..self.quadros.len()).map(|p| self.sessao.quadro_impresso(p).and_then(|b| Quadro::ler(&b)).unwrap_or_else(|_| self.quadros[p].clone())).collect()
    }

    fn atualizar(&mut self) {
        // O que vem de fora do documento: a data de hoje (campo `data`, o `\today`) e o nome do arquivo.
        let contexto = featherdocs_core::objetos::ContextoExterno {
            hoje: hoje_por_extenso(),
            arquivo: self.caminho.as_ref().and_then(|c| c.file_name()).map_or(String::new(), |n| n.to_string_lossy().into_owned()),
            ..Default::default()
        };
        self.sessao.contexto_definir(&contexto);
        let total = self.sessao.total_de_paginas();
        self.visao = self.sessao.visao(0, total.saturating_sub(1));
        self.pagina_ativa = self
            .visao
            .cursor
            .map_or(self.pagina_ativa, |c| c.pagina)
            .min(total.saturating_sub(1));
        // Página nova ou de tamanho trocado refaz tudo (o tamanho de cada uma posiciona a rolagem);
        // fora isso, a cada tecla só as páginas perto da vista.
        let cfg = self.sessao.documento().pagina;
        let tudo = self.quadros.len() != total || self.quadros.first().is_some_and(|q| q.largura != cfg.largura || q.altura != cfg.altura);
        if tudo {
            self.quadros.clear();
            self.versao_dos_quadros.clear();
            for p in 0..total {
                if let Some(q) = self.montar_quadro(p) {
                    self.quadros.push(q);
                    self.versao_dos_quadros.push(self.sessao.versao());
                }
            }
        } else {
            self.refazer_visiveis();
        }
        self.sincronizar_codigo();
    }

    /// Refaz os quadros velhos das páginas que estão perto da vista (e o da página do cursor).
    fn refazer_visiveis(&mut self) {
        // shortcut: a altura da vista é estimada (2000 px); com tela maior que isso, a página de baixo pode aparecer velha até a rolagem. Guardar o tamanho da janela se acontecer.
        const VISTA: f32 = 2000.;
        let versao = self.sessao.versao();
        let (topo, fim) = (self.rolagem.y - VISTA, self.rolagem.y + 2. * VISTA);
        let mut y = 24.;
        for p in 0..self.quadros.len() {
            let altura = self.quadros[p].altura * self.config.zoom;
            let perto = y + altura >= topo && y <= fim || p == self.pagina_ativa;
            y += altura + 24.;
            if perto && self.versao_dos_quadros[p] != versao {
                if let Some(q) = self.montar_quadro(p) {
                    self.quadros[p] = q;
                    self.versao_dos_quadros[p] = versao;
                }
            }
        }
    }

    /// O quadro da página `p` lido do motor, com as imagens dele já carregadas.
    fn montar_quadro(&mut self, p: usize) -> Option<Quadro> {
        match self.sessao.quadro(p, u64::MAX).and_then(|b| Quadro::ler(&b)) {
            Ok(q) => {
                for o in &q.operacoes {
                    if let Operacao::Imagem { origem, .. } = o {
                        if !self.imagens.contains_key(origem) {
                            if let Some(h) = crate::pagina::carregar_imagem(origem) {
                                self.imagens.insert(origem.clone(), h);
                            }
                        }
                    }
                }
                Some(q)
            }
            Err(e) => {
                self.status = e;
                None
            }
        }
    }
    fn carregar(&mut self, p: PathBuf, d: Documento) {
        self.config.lembrar(&p);
        self.sessao = Sessao::com_documento(d);
        // A sessão nova recomeça a contagem de versões: os quadros da anterior não valem mais.
        self.quadros.clear();
        self.sessao.marcar_gravado();
        // O texto de um .tex/.md aberto aparece no painel de código como a pessoa o escreveu.
        let ext = p.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase);
        let fonte = match ext.as_deref() {
            Some("tex") => Some(Fonte::Latex),
            Some("md") => Some(Fonte::Markdown),
            Some("zip") => Some(Fonte::Latex),
            _ => None,
        };
        // Do .zip, o texto é o do .tex principal de dentro. O LaTeX diz também o que não converteu.
        let lido = match ext.as_deref() {
            Some("zip") => std::fs::read(&p).ok().and_then(|b| featherdocs_io::latex::projeto::ler_zip(&b).ok()).map(|(l, t)| (t, l.avisos)),
            Some("tex") => std::fs::read_to_string(&p).ok().map(|t| {
                let avisos = featherdocs_io::latex::ler_na_pasta(&t, p.parent()).avisos;
                (t, avisos)
            }),
            _ => std::fs::read_to_string(&p).ok().map(|t| (t, Vec::new())),
        };
        let (texto, avisos) = lido.map_or((None, Vec::new()), |(t, a)| (Some(t), a));
        self.avisos_latex = avisos;
        self.fonte_original = fonte.zip(texto);
        self.arquivos_projeto = if fonte == Some(Fonte::Latex) { featherdocs_io::latex::projeto::arquivos_do_projeto(&p) } else { Vec::new() };
        self.projeto_caminho = (fonte == Some(Fonte::Latex)).then(|| p.clone());
        self.arquivo_visto = None;
        if let (Some(f), Some(painel)) = (fonte, self.codigo.as_mut()) {
            painel.fonte = f;
        }
        if let Some(painel) = self.codigo.as_mut() {
            painel.ultima_edicao = None;
            painel.versao = u64::MAX;
        }
        // Um documento importado é salvo como .fdoz, preservando o arquivo de origem.
        self.caminho = (ext.as_deref() == Some("fdoz")).then_some(p);
        self.imagens.clear();
        self.efeito_imagem = None;
        self.foco_documento = true;
        self.busca = false;
        self.sessao.busca_limpar();
        self.status.clear();
        self.atualizar();
    }
    pub(crate) fn solicitar(&mut self, d: Destino) -> Task<Msg> {
        if self.ocupado {
            return Task::none();
        }
        if self.sessao.estado().sujo {
            self.pendente = Some(d);
            self.foco_documento = false;
            Task::none()
        } else {
            self.executar_destino(d)
        }
    }
    fn executar_destino(&mut self, d: Destino) -> Task<Msg> {
        self.pendente = None;
        self.continuar_apos_salvar = false;
        match d {
            Destino::Novo => {
                self.inicio.tela_cheia = false;
                self.menu_aberto = false;
                self.sessao = Sessao::default();
                self.quadros.clear();
                self.caminho = None;
                self.fonte_original = None;
                if let Some(painel) = self.codigo.as_mut() {
                    painel.ultima_edicao = None;
                    painel.versao = u64::MAX;
                }
                self.imagens.clear();
                self.busca = false;
                self.foco_documento = true;
                self.status.clear();
                self.atualizar();
                Task::none()
            }
            Destino::Modelo(id) => {
                let Some(doc) = featherdocs_core::modelos::criar(id) else { return Task::none() };
                let t = self.executar_destino(Destino::Novo);
                self.sessao = Sessao::com_documento(doc);
                // A sessão nova começa na mesma versão da vazia: sem limpar, a página não é refeita.
                self.quadros.clear();
                self.atualizar();
                t
            }
            Destino::Fechar(_id) => Task::window(WindowCmd::Exit),
            Destino::AbrirCaminho(p) => {
                self.ocupado = true;
                Task::perform(async move { Self::carregar_em_background(p).await.map(Some) }, Msg::Aberto)
            }
            Destino::Abrir => {
                self.ocupado = true;
                Task::perform(
                    async {
                        let p = dialogo_de_arquivo()
                            .add_filter("Documentos", &["fdoz", "docx", "md", "tex", "zip"])
                            .pick_file()
                            .await;
                        match p {
                            Some(p) => Self::carregar_em_background(p.path().to_path_buf())
                                .await
                                .map(Some),
                            None => Ok(None),
                        }
                    },
                    Msg::Aberto,
                )
            }
        }
    }
    fn salvar(&mut self, como: bool, exportar: bool) -> Task<Msg> {
        let p = if como || exportar { None } else { self.caminho.clone() };
        self.salvar_em(p, exportar, false)
    }
    /// Salva (ou exporta) em `p`, ou no arquivo que a pessoa escolher; `confirmado` = ela já viu
    /// o aviso do que o formato não leva.
    fn salvar_em(&mut self, p: Option<PathBuf>, exportar: bool, confirmado: bool) -> Task<Msg> {
        if self.ocupado {
            return Task::none();
        }
        self.ocupado = true;
        // A vertente do documento: o .tex/.md aberto é código; o resto é o documento da página.
        let vertente = self.vertente();
        let doc = self.sessao.documento().clone();
        let versao = self.sessao.versao();
        // O PDF sai das páginas já compostas (as mesmas da tela), não do documento.
        let quadros = if exportar { self.quadros_impressos() } else { Vec::new() };
        let (links, marcadores) = if exportar { (self.sessao.links_do_pdf(), self.sessao.marcadores_do_pdf()) } else { Default::default() };
        let opcoes_pdf = crate::pdf::OpcoesPdf::do_documento(&doc.meta.titulo, &doc.meta.autor);
        let origem = self.caminho.clone();
        Task::perform(
            async move {
                let p = if let Some(p) = p {
                    Some(p)
                } else {
                    let dialogo = dialogo_de_arquivo().set_file_name(if exportar {
                        "documento.docx"
                    } else {
                        "documento.fdoz"
                    });
                    let dialogo = if exportar {
                        dialogo
                            .add_filter("Word", &["docx"])
                            .add_filter("PDF", &["pdf"])
                            .add_filter("Markdown", &["md"])
                            .add_filter("LaTeX", &["tex"])
                            .add_filter(tr("Projeto LaTeX (.zip)"), &["zip"])
                    } else {
                        dialogo.add_filter("FeatherDocs", &["fdoz"])
                    };
                    dialogo.save_file().await.map(|f| f.path().to_path_buf())
                };
                let Some(mut p) = p else {
                    return Ok(Ok(None));
                };
                if p.extension().is_none() {
                    p.set_extension(if exportar { "docx" } else { "fdoz" });
                    if p.exists() {
                        return Err(tr("O arquivo já existe. Selecione seu nome completo no diálogo para confirmar a substituição.").into());
                    }
                }
                if !exportar
                    && !p
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.eq_ignore_ascii_case("fdoz"))
                {
                    return Err(tr("Para salvar o documento, escolha um arquivo .fdoz. Use Exportar para outros formatos.").into());
                }
                if exportar && !confirmado {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                    let perdas = featherdocs_io::conversao::perdas(&doc, vertente, &ext);
                    if !perdas.is_empty() {
                        return Ok(Err((p, perdas)));
                    }
                }
                // Serialização e escrita podem durar segundos; executar em worker.
                tokio::task::spawn_blocking(move || {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("docx");
                    let b = if ext.eq_ignore_ascii_case("pdf") {
                        crate::pdf::gerar_com(&quadros, &opcoes_pdf, &links, &marcadores)?
                    } else if ext.eq_ignore_ascii_case("fdoz") {
                        arquivos::com_historico(arquivos::bytes(&doc, ext)?, &p, origem.as_deref())?
                    } else {
                        arquivos::bytes(&doc, ext)?
                    };
                    arquivos::gravar(&p, &b)?;
                    Ok::<_, String>(Some((p, versao, exportar)))
                })
                .await
                .map_err(|e| trf("Falha ao salvar documento: {}", &[&e]))?
                .map(Ok)
            },
            |r: Result<Result<Option<(PathBuf, u64, bool)>, (PathBuf, Vec<&'static str>)>, String>| match r {
                Ok(Err((p, perdas))) => Msg::PerdasAoExportar(p, perdas),
                Ok(Ok(x)) => Msg::Gravado(Ok(x)),
                Err(e) => Msg::Gravado(Err(e)),
            },
        )
    }
    /// O aviso antes de exportar para um formato que não leva tudo, com a lista do que fica de fora.
    fn view_perdas<'a>(&'a self, p: Option<&'a std::path::Path>, perdas: &'a [&'static str]) -> Element<'a, Msg> {
        let nome = p.and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        let mut lista = column![].spacing(6);
        for chave in perdas {
            lista = lista.push(row![icone("alert-triangle", 14.), text(tr(featherdocs_io::conversao::descricao(chave))).size(13)].spacing(8).align(Align::Center));
        }
        container(
            column![
                text(tr("Este formato não leva tudo do documento")).size(19),
                text(match &nome {
                    Some(nome) => trf("Ao exportar {}, isto fica de fora:", &[nome]),
                    None => tr("O código em Markdown não tem isto; o que for editado nele perde:").into(),
                })
                .size(13),
                lista,
                row![
                    button(text(tr(if nome.is_some() { "Exportar mesmo assim" } else { "Ver em Markdown mesmo assim" })))
                        .on_press(Msg::ExportarMesmoAssim)
                        .style(estilo_botao_primario),
                    button(text(tr("cancelar"))).on_press(Msg::FecharPerdas).style(estilo_botao_secundario),
                ]
                .spacing(10)
            ]
            .spacing(15),
        )
        .max_width(640. + 60.)
        .padding(30u16)
        .width(Length::FILL)
        .height(Length::FILL)
        .into()
    }
    /// Cada mensagem; o que mudou na barra de status vai também para o Console do F12.
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        let antes = self.status.clone();
        // O editor do pelicano rola sozinho até o cursor (o do Iced ficava numa barra de rolagem de fora).
        let tarefa = self.update_interno(msg);
        if self.status != antes && !self.status.is_empty() {
            let nivel = feather_ui::monitor::Nivel::da_mensagem(&self.status);
            self.monitor.registrar(nivel, "FeatherDocs", &self.status.clone());
            return Task::batch([tarefa, self.monitor.acompanhar()]);
        }
        tarefa
    }
    fn update_interno(&mut self, msg: Msg) -> Task<Msg> {
        if (self.ocupado || self.pendente.is_some())
            && matches!(
                msg,
                Msg::Clicar(..)
                    | Msg::Tecla(..)
                    | Msg::Colado(..)
                    | Msg::ColadoPuro(..)
                    | Msg::BotaoDireito(..)
                    | Msg::CtrlClique(..)
                    | Msg::Fonte(..)
                    | Msg::Tamanho(..)
                    | Msg::CorEscolhida(..)
                    | Msg::AbrirEfeitosImagem
                    | Msg::AplicarEfeitoImagem(..)
                    | Msg::IrBloco(..)
                    | Msg::Consulta(..)
                    | Msg::Substituicao(..)
                    | Msg::BuscaIr(..)
                    | Msg::Substituir(..)
            )
        {
            return Task::none();
        }
        match msg {
            Msg::Inicio(e) => return self.evento_inicio(e),
            Msg::EstruturaArrastar => self.arrastando_estrutura = true,
            Msg::EstruturaMover(x) if self.arrastando_estrutura => {
                // Encostar no canto fecha o painel; voltar (sem soltar) abre de novo.
                self.config.estrutura = x >= LARGURA_ESTRUTURA_SOME;
                if self.config.estrutura {
                    self.config.largura_estrutura = x.clamp(LARGURA_ESTRUTURA_MIN, (self.tamanho_principal.width * 0.5).max(LARGURA_ESTRUTURA_MIN));
                }
            }
            Msg::EstruturaMover(_) => {}
            Msg::EstruturaSoltar => {
                self.arrastando_estrutura = false;
                self.config.gravar();
            }
            Msg::BoasVindas(e) => return self.evento_boas_vindas(e),
            // O editor do pelicano já mudou o texto: marca o código como pendente (aplicado depois da pausa).
            // shortcut: o pelicano edita o `Content` direto; com o app ocupado a edição não é barrada (o
            // Iced descartava a ação), ela só espera e é aplicada depois. Barrar pede editor só leitura no pelicano.
            Msg::CodigoMudou => {
                self.foco_documento = false;
                if let Some(painel) = self.codigo.as_mut() {
                    painel.ultima_edicao = Some(Instant::now());
                }
                return Task::none();
            }
            Msg::CodigoFoco => {
                self.foco_documento = false;
                return Task::none();
            }
            Msg::CodigoCursor => return self.codigo_cursor(),
            Msg::JanelaRedimensionada(t) => {
                self.tamanho_principal = t;
                return Task::none();
            }
            Msg::CodigoPulso => {
                if !self.codigo.as_ref().is_some_and(|p| p.maduro(Instant::now())) {
                    return Task::none();
                }
                self.aplicar_codigo();
            }
            Msg::CapturarTela => return self.capturar_tela(),
            // Qualquer outra ação aplica antes o código que ainda esperava a pausa: a página
            // nunca é editada por cima de um código que vai substituí-la depois.
            _ => self.aplicar_codigo(),
        }
        let mut tarefa = Task::none();
        match msg {
            Msg::Comando(c) => {
                self.menu_faixa = None;
                return self.comando(c);
            }
            Msg::MenuFaixa(m) => {
                self.menu_faixa = m;
                self.ponto_menu_faixa = self.ponteiro_faixa;
                return Task::none();
            }
            Msg::PonteiroFaixa(p) => {
                self.ponteiro_faixa = p;
                return Task::none();
            }
            Msg::Desenhar(e) => {
                self.evento_desenhar(e);
                return Task::none();
            }
            Msg::Tinta(pagina, e) => {
                self.evento_tinta(pagina, e);
            }
            Msg::GaleriaEstilos(passo) => {
                self.galeria_estilos = self.galeria_estilos.saturating_add_signed(passo as isize);
            }
            Msg::CodigoFechar => {
                self.codigo = None;
                self.foco_documento = true;
            }
            Msg::Atalho(c) => {
                if matches!(c, Comando::Copiar | Comando::Recortar | Comando::Colar | Comando::ColarSemFormatacao)
                    && !self.foco_documento
                {
                    return Task::none();
                }
                return self.comando(c);
            }
            Msg::Fechar(id) => {
                if Some(id) == self.janelas.principal {
                    return self.solicitar(Destino::Fechar(id));
                }
                self.janelas.retirar(id);
                return Task::window_of(id, WindowCmd::Close);
            }
            Msg::FecharAuxiliar(tipo) => return self.janelas.fechar_tipo(tipo),
            Msg::ArrastarJanela(id) => return feather_ui::janelas::arrastar_ou_maximizar(id, &mut self.ultimo_clique_barra),
            Msg::Minimizar(id) => return Task::window_of(id, WindowCmd::Minimize),
            Msg::AlternarMaximizar(id) => return Task::window_of(id, WindowCmd::ToggleMaximize),
            Msg::AtalhoJanela(id, key, mods) => {
                if let Some(tipo) = self.janelas.tipo(id) {
                    // A Ajuda TS usava Ctrl+F para focalizar a pesquisa.
                    if tipo == JanelaAux::Ajuda && mods == keyboard::Modifiers::CONTROL
                        && matches!(&key, keyboard::Key::Character(c)
                            if c.as_str().eq_ignore_ascii_case("f")) {
                        return feather_ui::ajuda::focar_busca::<Msg>();
                    }
                    use feather_ui::atalhos::PainelGlobal;
                    let painel = match tipo {
                        JanelaAux::Ajuda => PainelGlobal::Ajuda,
                        JanelaAux::Monitor => PainelGlobal::Monitor,
                        JanelaAux::Configuracoes | JanelaAux::PrimeirosPassos | JanelaAux::Novidades => PainelGlobal::Configuracoes,
                    };
                    if feather_ui::atalhos::fechar_painel(painel, &key, mods) {
                        return self.janelas.fechar_tipo(tipo);
                    }
                    return Task::none();
                }
                if let Some(acao) = feather_ui::atalhos::acao_global(&key, mods) {
                    use feather_ui::atalhos::AcaoGlobal;
                    return match acao {
                        AcaoGlobal::Ajuda => self.comando(Comando::Ajuda),
                        AcaoGlobal::Monitor => self.comando(Comando::Monitor),
                        AcaoGlobal::AlternarFaixa => self.update(Msg::AlternarFaixa),
                    };
                }
                return match feather_ui::atalhos::combo_de(&key, mods).as_deref() {
                    Some("Escape") => self.update(Msg::Cancelar),
                    Some("Ctrl+N") => self.comando(Comando::Novo),
                    Some("Ctrl+O") => self.comando(Comando::Abrir),
                    Some("Ctrl+Shift+S") => self.comando(Comando::SalvarComo),
                    Some("Ctrl+S") => self.comando(Comando::Salvar),
                    Some("Ctrl+P") => self.comando(Comando::ExportarPdf),
                    Some("Ctrl+C") => self.update(Msg::Atalho(Comando::Copiar)),
                    Some("Ctrl+X") => self.update(Msg::Atalho(Comando::Recortar)),
                    Some("Ctrl+V") => self.update(Msg::Atalho(Comando::Colar)),
                    Some("Ctrl+Shift+V") => self.update(Msg::Atalho(Comando::ColarSemFormatacao)),
                    Some("Ctrl+F") => self.comando(Comando::Buscar),
                    Some("Ctrl+K") => self.update(Msg::FormAbrir(crate::dialogos::Tipo::Link)),
                    Some("Ctrl+,") => self.comando(Comando::Menu),
                    _ => Task::none(),
                };
            }
            Msg::BarraPropria(valor) => {
                self.config.barra_propria = valor;
                self.config.gravar();
            }
            Msg::NumerosDeLinha(f, valor) => {
                self.config.numeros_de_linha[f] = valor;
                self.config.gravar();
            }
            Msg::TemaCodigo(f, i) => {
                self.config.tema_codigo[f] = i;
                self.config.gravar();
            }
            Msg::Lateral(v) => self.lateral = v,
            Msg::IrLinhaCodigo(n) => {
                self.arquivo_visto = None;
                if self.fonte_do_codigo() != Some(Fonte::Latex) {
                    self.alternar_codigo(Fonte::Latex);
                }
                if let Some(painel) = self.codigo.as_mut() {
                    painel.ir_a_linha(n.saturating_sub(1));
                    // A página vai junto, ao bloco daquela linha.
                    if let Some(id) = painel.bloco_da_linha(n.saturating_sub(1)) {
                        painel.bloco_marcado = Some(id);
                        self.sessao.ir_para_bloco(id);
                        tarefa = self.rolar_ao_cursor();
                    }
                }
            }
            Msg::AbrirArquivo(nome) => {
                if nome.to_ascii_lowercase().ends_with(".tex") {
                    self.arquivo_visto = None;
                    if self.fonte_do_codigo() != Some(Fonte::Latex) {
                        self.alternar_codigo(Fonte::Latex);
                    }
                } else {
                    let bytes = self.projeto_caminho.as_deref().and_then(|p| featherdocs_io::latex::projeto::ler_arquivo(p, &nome));
                    let imagem = ["png", "jpg", "jpeg", "gif", "webp", "svg"].iter().any(|x| nome.to_ascii_lowercase().ends_with(&format!(".{x}")));
                    let visto = match bytes {
                        None => Visto::Aviso(tr("Não consegui ler este arquivo do projeto.").into()),
                        Some(b) if imagem => crate::imagem_compartilhada::original_para_zoom(&b)
                            // shortcut: SVG/GIF vão pela prévia com tamanho suposto de 800×600; medir o SVG se precisar.
                            .or_else(|| crate::pagina::carregar_imagem(&featherdocs_io::latex::projeto::data_url_da_imagem(&nome, &b)).map(|h| (h, 800, 600)))
                            .map_or_else(|| Visto::Aviso(tr("Esta imagem não pode ser mostrada.").into()), |(h, w, a)| Visto::Imagem(h, w, a)),
                        Some(b) => match String::from_utf8(b) {
                            Ok(t) => Visto::Texto(t),
                            Err(_) => Visto::Aviso(tr("Arquivo binário: não há texto para mostrar.").into()),
                        },
                    };
                    self.arquivo_visto = Some((nome, visto));
                    self.zoom_arquivo = 1.;
                }
            }
            Msg::FecharArquivo => self.arquivo_visto = None,
            // 0 = ajustar ao painel; senão multiplica (1 = a imagem inteira cabendo no painel).
            Msg::ZoomArquivo(f) => self.zoom_arquivo = if f == 0. { 1. } else { (self.zoom_arquivo * f).clamp(0.1, 20.) },
            Msg::SalvarEContinuar => {
                self.continuar_apos_salvar = true;
                return self.salvar(false, false);
            }
            Msg::Descartar => {
                if let Some(d) = self.pendente.take() {
                    return self.executar_destino(d);
                }
            }
            Msg::Cancelar => {
                if self.sessao.pincel_cheio() {
                    self.sessao.pincel_cancelar();
                    self.status = tr("Pincel cancelado").into();
                }
                self.pendente = None;
                self.opcoes_pdf = None;
                self.menu_faixa = None;
                self.continuar_apos_salvar = false;
                self.menu_aberto = false;
                self.efeito_imagem = None;
                self.formulario = None;
                self.menu_contexto = None;
                self.busca = false;
                self.sessao.busca_limpar();
                self.foco_documento = true;
            }
            Msg::Aba(a) => self.aba = a,
            Msg::Escuro(b) => {
                self.config.tema_auto = false;
                self.config.escuro = b;
                definir_tema(b, self.config.acento);
                self.config.gravar();
            }
            Msg::TemaAuto => {
                self.config.tema_auto = true;
                definir_tema(tema_escuro(true, self.config.escuro), self.config.acento);
                self.config.gravar();
            }
            Msg::Acento(i) => {
                self.config.acento = i.min(ACENTOS.len() - 1);
                self.config.marca_personalizada = Some(true);
                definir_tema(tema_escuro(self.config.tema_auto, self.config.escuro), self.config.acento);
                feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherDocs,true);
                self.config.gravar();
            }
            Msg::Idioma(codigo) => {
                self.config.idioma = codigo.to_string();
                crate::localidade::definir(codigo);
                self.config.gravar();
                return Task::none();
            }
            Msg::LogoPadrao => {
                self.config.marca_personalizada = Some(false);
                feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherDocs,false);
                self.config.gravar();
            }
            Msg::ConfigSecao(s) => self.config_secao = s.min(7),
            Msg::AjudaEvento(e) => self.ajuda_estado.atualizar(e, ajuda::modelo()),
            Msg::AlternarFaixa => {
                self.menu_faixa = None;
                self.config.faixa_recolhida = !self.config.faixa_recolhida;
                self.config.gravar();
            },
            Msg::Aberto(r) => {
                self.ocupado = false;
                match r {
                    Ok(Some((p, d))) => {
                        self.inicio.tela_cheia = false;
                        self.carregar(p, d)
                    }
                    Ok(None) => {
                        self.status = tr("Abertura cancelada; documento preservado").into();
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::PerdasAoExportar(p, perdas) => {
                self.ocupado = false;
                self.perdas_exportar = Some((Some(p), perdas));
            }
            Msg::ExportarMesmoAssim => {
                match self.perdas_exportar.take() {
                    Some((Some(p), _)) => return self.salvar_em(Some(p), true, true),
                    Some((None, _)) => self.mostrar_codigo(Fonte::Markdown),
                    None => {}
                }
            }
            Msg::FecharPerdas => self.perdas_exportar = None,
            Msg::OpcoesPdf(o) => self.opcoes_pdf = Some(o),
            Msg::ConfirmarPdf => {
                if let Some(o) = self.opcoes_pdf.take() {
                    return self.exportar_pdf(o);
                }
            }
            Msg::Gravado(r) => {
                self.ocupado = false;
                match r {
                    Ok(Some((p, v, exportar))) => {
                        let pdf = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
                        if !exportar {
                            self.config.lembrar(&p);
                            self.caminho = Some(p);
                            if self.sessao.versao() == v {
                                self.sessao.marcar_gravado();
                            }
                        }
                        self.status = tr(if exportar { "exportado" } else { "salvo" }).into();
                        if exportar && pdf {
                            if let Some(a) = crate::pdf::aviso(&self.quadros) {
                                self.status = a;
                            }
                        }
                        if self.continuar_apos_salvar && !self.sessao.estado().sujo {
                            if let Some(d) = self.pendente.take() {
                                return self.executar_destino(d);
                            }
                        }
                    }
                    Ok(None) => self.continuar_apos_salvar = false,
                    Err(e) => {
                        self.status = e;
                        self.continuar_apos_salvar = false;
                    }
                }
            }
            Msg::AbrirEfeitosImagem => {
                if self.efeito_imagem.is_some() {
                    self.efeito_imagem = None;
                } else {
                    let id = self.sessao.cursor().foco.bloco;
                    let origem = self.sessao.documento().blocos.iter()
                        .find(|b| b.id == id)
                        .and_then(|b| match &b.conteudo {
                            featherdocs_core::modelo::Conteudo::Imagem { origem, .. }
                                if origem.starts_with("data:image/png;base64,")
                                || origem.starts_with("data:image/jpeg;base64,") => Some(origem.clone()),
                            _ => None,
                        });
                    match origem {
                        Some(origem) => self.efeito_imagem = Some((id, origem, "original")),
                        None => self.status = tr("Selecione uma imagem PNG/JPEG embutida para editar.").into(),
                    }
                }
            }
            Msg::AplicarEfeitoImagem(chave) => {
                if let Some((id, origem, _)) = self.efeito_imagem.clone() {
                    let result = feather_image::EfeitoRapido::da_chave(chave)
                        .ok_or_else(|| tr("Efeito de imagem desconhecido.").to_string())
                        .and_then(|preset|
                            crate::imagem_compartilhada::aplicar_efeito_embutido(&origem, preset)
                        )
                        .and_then(|nova| self.sessao.imagem_origem_atualizar(id, &nova).map(|_| ()));
                    match result {
                        Ok(()) => {
                            self.imagens.clear();
                            self.efeito_imagem = Some((id, origem, chave));
                            self.status = trf("Efeito de imagem aplicado: {}", &[&chave]);
                        }
                        Err(erro) => self.status = erro,
                    }
                }
            }
            Msg::FecharEfeitosImagem => self.efeito_imagem = None,
            Msg::Imagem(r) => {
                self.ocupado = false;
                match r {
                    Ok(Some((origem, w, h))) => {
                        if let Err(e) = self.sessao.inserir_imagem(&origem, w, h) {
                            self.status = e;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => self.status = e,
                }
            }
            Msg::Clicar(p, x, y, shift, n) if !self.ocupado && self.pendente.is_none() => {
                self.foco_documento = true;
                self.arrastando = true;
                let versao = self.sessao.versao();
                // A página manda: o cursor do código vai ao bloco clicado (em `atualizar`).
                if let Some(painel) = self.codigo.as_mut() {
                    painel.bloco_marcado = None;
                }
                // Duplo clique numa nota abre o texto dela para editar.
                if !shift && n == 2 {
                    if let Some(tipo) = self.nota_em(p, x, y) {
                        return self.update(Msg::FormAbrir(tipo));
                    }
                }
                // Clique simples na caixa de um item de tarefa marca/desmarca, sem mover o cursor.
                if shift || n != 1 || self.sessao.clicar_tarefa(p, x, y).is_none() {
                    self.sessao.clicar(p, x, y, shift, n);
                }
                self.pagina_ativa = p;
                if shift && self.sessao.pincel_cheio() && !self.sessao.cursor().vazio() {
                    self.sessao.pincel_aplicar();
                    self.sessao.pincel_cancelar();
                    self.status = tr("Pincel aplicado à seleção").into();
                }
                if self.sessao.versao() == versao && self.quadros.len() == self.sessao.total_de_paginas() {
                    self.atualizar_cursor();
                    return Task::none();
                }
            }
            Msg::MonitorEvento(e) => return self.monitor.atualizar(e),
            Msg::MonitorPulso => {
                self.monitor.amostrar();
                return Task::none();
            }
            Msg::Soltar => {
                self.arrastando = false;
                return Task::none();
            }
            Msg::Tecla(k, m, t)
                if self.foco_documento && !self.ocupado && self.pendente.is_none() =>
            {
                // Modo desenho: Esc para de desenhar; Delete apaga os traços do laço.
                if self.tinta.ui.ativo && k == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.evento_desenhar(feather_ui::tinta::Evento::Parar);
                    return Task::none();
                }
                if matches!(k, keyboard::Key::Named(keyboard::key::Named::Delete | keyboard::key::Named::Backspace))
                    && self.apagar_selecao_tinta()
                {
                    return Task::none();
                }
                if m.control() && !m.shift() && matches!(&k, keyboard::Key::Character(c) if c.as_str().eq_ignore_ascii_case("z"))
                    && self.desfazer_tinta()
                {
                    return Task::none();
                }
                if self.sessao.pincel_cheio()
                    && k == keyboard::Key::Named(keyboard::key::Named::Escape)
                {
                    self.sessao.pincel_cancelar();
                    self.status = tr("Pincel cancelado").into();
                    return Task::none();
                }
                let nome = nome_tecla(&k);
                let antes = (self.sessao.versao(), *self.sessao.cursor());
                let r = self.sessao.tecla(&nome, m.control(), m.shift(), m.alt());
                if !r.tratada && (!m.control() || m.alt()) {
                    if let Some(t) = t.filter(|t| !t.chars().any(char::is_control)) {
                        self.sessao.inserir_texto(&t);
                    }
                }
                // Só segue o cursor se a tecla mexeu nele ou no texto: o Ctrl sozinho (do zoom com a
                // roda) levava a tela de volta ao cursor, que depois de abrir fica na primeira página.
                if antes != (self.sessao.versao(), *self.sessao.cursor()) {
                    tarefa = self.rolar_ao_cursor();
                }
            }
            Msg::Colado(Some(t)) if !self.ocupado && self.pendente.is_none() => {
                self.sessao.colar(&t);
            }
            Msg::Comparado(r) => {
                self.ocupado = false;
                match r {
                    Ok(Some((p, antigo))) => {
                        let rotulo = p.file_name().and_then(|n| n.to_str()).unwrap_or(tr("outro arquivo")).to_string();
                        self.sessao.comparar_com(&antigo, &rotulo);
                        self.painel_revisao = true;
                        self.status = trf("Diferenças em relação a {} marcadas como alterações", &[&rotulo]);
                    }
                    Ok(None) => {}
                    Err(e) => self.status = e,
                }
            }
            Msg::ComentarioIr(id) => {
                self.sessao.comentario_ir(id);
                self.foco_documento = true;
                tarefa = self.rolar_ao_cursor();
            }
            Msg::ComentarioResolver(id, sim) => {
                if let Err(e) = self.sessao.comentario_mudar(id, None, None, Some(sim)) {
                    self.status = e;
                }
            }
            Msg::ComentarioExcluir(id) => {
                self.sessao.comentario_excluir(id);
            }
            // Rolar não muda o documento: nada de redesenhar as páginas a cada passo da roda.
            Msg::Rolou(v) => {
                self.rolagem = v;
                self.refazer_visiveis();
                return Task::none();
            }
            Msg::BotaoDireito(p, x, y, abs) => {
                self.foco_documento = true;
                self.pagina_ativa = p;
                self.nota_contexto = self.nota_em(p, x, y);
                self.sessao.clicar_direito(p, x, y);
                // O pelicano já dá o ponto na janela (o Iced dava no conteúdo rolado e descontava a rolagem).
                self.menu_contexto = Some(abs);
            }
            Msg::FecharContexto => self.menu_contexto = None,
            Msg::AbrirLink => {
                if let Some(link) = self.sessao.link_no_cursor() {
                    self.seguir_link(&link);
                    tarefa = self.rolar_ao_cursor();
                }
            }
            Msg::DoMenu(m) => {
                self.menu_contexto = None;
                self.menu_faixa = None;
                return self.update(*m);
            }
            Msg::CtrlClique(p, x, y) => {
                self.foco_documento = true;
                self.pagina_ativa = p;
                // A linha do sumário leva ao título dela (como o link do `\ref`).
                if let Some(id) = self.sessao.alvo_do_sumario(p, y) {
                    self.sessao.ir_para_bloco(id);
                    return self.rolar_ao_cursor();
                }
                self.sessao.clicar(p, x, y, false, 1);
                if let Some(link) = self.sessao.link_no_cursor() {
                    self.seguir_link(&link);
                    // O link interno leva o cursor ao destino; a tela vai junto.
                    tarefa = self.rolar_ao_cursor();
                }
            }
            Msg::BibLido(r) => {
                self.ocupado = false;
                match r {
                    Ok(Some(t)) => {
                        let fontes = featherdocs_io::bib::ler(&t);
                        if fontes.is_empty() {
                            self.status = tr("Nenhuma referência reconhecida no .bib").into();
                        } else {
                            let (_, n) = self.sessao.fontes_importar(fontes);
                            self.status = trf("{} fonte(s) importada(s) do .bib", &[&n]);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => self.status = e,
                }
            }
            Msg::AbrirRecente(p) => {
                self.menu_aberto = false;
                return self.solicitar(Destino::AbrirCaminho(p));
            }
            Msg::FormAbrir(tipo) => {
                self.formulario = Some(crate::dialogos::Formulario::novo(tipo, &self.sessao));
                self.foco_documento = false;
                return Task::focus("form0");
            }
            Msg::FormCampo(i, v) => {
                if let Some(c) = self.formulario.as_mut().and_then(|f| f.campos.get_mut(i)) {
                    *c = v;
                }
            }
            Msg::FormOk => {
                if let Some(f) = &self.formulario {
                    match f.aplicar(&mut self.sessao) {
                        Ok(recado) => {
                            self.status = recado;
                            self.formulario = None;
                            self.foco_documento = true;
                        }
                        // O diálogo fica aberto para a pessoa corrigir o campo.
                        Err(e) => self.status = e,
                    }
                }
            }
            Msg::FormFechar => {
                self.formulario = None;
                self.foco_documento = true;
            }
            Msg::FormRemoverLink => {
                self.sessao.link_remover();
                self.formulario = None;
                self.foco_documento = true;
            }
            Msg::FormEscolher(id) => {
                let modo = self.formulario.as_ref().map_or("texto".to_string(), |f| f.campos[0].clone());
                match self.sessao.referencia_inserir(id, &modo) {
                    Ok(_) => {
                        self.formulario = None;
                        self.foco_documento = true;
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::ColadoPuro(Some(t)) if !self.ocupado && self.pendente.is_none() => {
                self.sessao.colar_sem_formatacao(&t);
            }
            Msg::CorEscolhida(alvo, cor) => {
                let resultado = match alvo {
                    AlvoCor::Texto => self.sessao.cor_definir(&cor),
                    AlvoCor::Realce => self.sessao.realce_definir(&cor),
                    AlvoCor::Fundo => self.sessao.fundo_definir(&cor),
                    AlvoCor::Pagina => self.sessao.cor_pagina_definir(&cor),
                };
                if let Err(erro) = resultado {
                    self.status = erro;
                }
            }
            Msg::Fonte(f) if !self.ocupado => {
                if let Err(e) = self.sessao.fonte_definir(&f) {
                    self.status = e;
                }
            }
            Msg::Tamanho(t) if !self.ocupado => {
                if let Err(e) = self.sessao.tamanho_definir(t as f32) {
                    self.status = e;
                }
            }
            Msg::IrBloco(id) => {
                self.sessao.ir_para_bloco(id);
                self.foco_documento = true;
                tarefa = self.rolar_ao_cursor();
            }
            Msg::BuscaOpcao(i) => {
                let o = &mut self.busca_opcoes;
                match i {
                    0 => o.maiusculas = !o.maiusculas,
                    1 => o.palavra = !o.palavra,
                    _ => o.regex = !o.regex,
                }
                return self.update(Msg::Consulta(self.consulta.clone()));
            }
            Msg::Consulta(s) => {
                self.consulta = s;
                self.foco_documento = false;
                let r = self.sessao.busca_definir(&self.consulta, self.busca_opcoes);
                self.busca_contagem = match (r.erro, r.total) {
                    (Some(e), _) => e,
                    (None, 0) if !self.consulta.is_empty() => tr("Nenhum resultado").into(),
                    (None, 0) => String::new(),
                    (None, t) => trf("{} de {}", &[&r.atual.to_string(), &t.to_string()]),
                };
                tarefa = self.rolar_ao_cursor();
            }
            Msg::AlternarSubstituir => self.substituir_visivel = !self.substituir_visivel,
            Msg::Substituicao(s) => {
                self.substituicao = s;
                self.foco_documento = false;
            }
            Msg::BuscaIr(d) => {
                let r = self.sessao.busca_ir(d);
                if r.total > 0 {
                    self.busca_contagem = trf("{} de {}", &[&r.atual.to_string(), &r.total.to_string()]);
                }
                tarefa = self.rolar_ao_cursor();
            }
            Msg::Substituir(tudo) => {
                if tudo {
                    self.sessao.substituir_tudo(&self.substituicao);
                } else {
                    self.sessao.substituir(&self.substituicao);
                }
            }
            _ => {}
        }
        self.atualizar();
        tarefa
    }
    fn comando(&mut self, c: Comando) -> Task<Msg> {
        if self.ocupado || self.pendente.is_some() {
            return Task::none();
        }
        let mut tarefa = Task::none();
        match c {
            Comando::Novo => return self.solicitar(Destino::Novo),
            Comando::Abrir => return self.solicitar(Destino::Abrir),
            Comando::Salvar => return self.salvar(false, false),
            Comando::SalvarComo => return self.salvar(true, false),
            Comando::Exportar => return self.salvar(true, true),
            Comando::ExportarPdf => {
                let meta = &self.sessao.documento().meta;
                self.menu_aberto = false;
                self.opcoes_pdf = Some(crate::pdf::OpcoesPdf::do_documento(&meta.titulo, &meta.autor));
            }
            Comando::Desfazer => {
                if !self.desfazer_tinta() {
                    self.sessao.desfazer();
                }
            }
            Comando::Refazer => {
                self.sessao.refazer();
            }
            Comando::Copiar => tarefa = Task::write_clipboard(self.sessao.copiar()),
            Comando::Recortar => {
                let (t, _) = self.sessao.recortar();
                tarefa = Task::write_clipboard(t);
            }
            Comando::Colar => return Task::read_clipboard(Msg::Colado),
            Comando::ColarSemFormatacao => return Task::read_clipboard(Msg::ColadoPuro),
            Comando::NumeroPagina(modelo, onde) => {
                if let Err(e) = self.sessao.numero_de_pagina(modelo, onde) {
                    self.status = e;
                }
            }
            Comando::Formulario(tipo) => return self.update(Msg::FormAbrir(tipo)),
            Comando::ImportarBib => {
                self.ocupado = true;
                return Task::perform(
                    async {
                        let Some(p) = dialogo_de_arquivo().add_filter("BibTeX", &["bib"]).pick_file().await else {
                            return Ok(None);
                        };
                        std::fs::read_to_string(p.path()).map(Some).map_err(|e| trf("Não consegui ler o .bib: {}", &[&e]))
                    },
                    Msg::BibLido,
                );
            }
            Comando::Bibliografia(todas) => {
                self.sessao.bibliografia_inserir(todas);
            }
            Comando::EstiloCitacao(nome) => {
                if let Err(e) = self.sessao.estilo_citacao_definir(nome) {
                    self.status = e;
                }
            }
            Comando::NumeroRemover => {
                self.sessao.numero_remover();
            }
            Comando::Legenda(tipo) => {
                if let Err(e) = self.sessao.legenda_inserir(tipo) {
                    self.status = e;
                }
            }
            Comando::IndiceInserir => {
                self.sessao.indice_inserir();
            }
            Comando::ListaDe(n) => {
                self.sessao.lista_inserir(n);
            }
            Comando::SumarioRemover => {
                self.sessao.remover_sumario();
            }
            Comando::SumarioNiveis(n) => {
                self.sessao.sumario_configurar(n, true, true);
            }
            Comando::ReferenciasAtualizar => {
                self.sessao.referencias_atualizar();
            }
            Comando::Margens(cm) => {
                let [e, d, s, i] = cm.map(|v| v * 72.0 / 2.54);
                if let Err(erro) = self.sessao.margens_definir(e, d, s, i) {
                    self.status = erro;
                }
            }
            Comando::NumerarLinhas(nome) => {
                if let Err(e) = self.sessao.numerar_linhas_definir(nome) {
                    self.status = e;
                }
            }
            Comando::PrimeiraDiferente => {
                let sim = !self.sessao.documento().extras.primeira_diferente;
                self.sessao.primeira_diferente_definir(sim);
            }
            Comando::MarcaDagua(t) => {
                if let Err(e) = self.sessao.marca_dagua_definir(t, "") {
                    self.status = e;
                }
            }
            Comando::Vertical(nome) => {
                if let Err(e) = self.sessao.vertical_definir(nome) {
                    self.status = e;
                }
            }
            Comando::Proteger => {
                let sim = !self.sessao.documento().extras.protegido;
                self.sessao.proteger(sim);
                self.status = tr(if sim { "Documento protegido: só leitura e comentários" } else { "Proteção retirada" }).into();
            }
            Comando::Posicao(p) => {
                self.sessao.posicao_alternar(p);
            }
            Comando::TamanhoPasso(d) => {
                self.sessao.tamanho_passo(d);
            }
            Comando::LimparFormatacao => {
                self.sessao.limpar_formatacao();
            }
            Comando::Recuar(d) => {
                self.sessao.recuar(d);
            }
            Comando::ListaReiniciar => {
                self.sessao.lista_reiniciar();
            }
            Comando::Borda(lados) => {
                self.sessao.borda_definir(lados);
            }
            Comando::Maiusculas(modo) => {
                if let Err(e) = self.sessao.maiusculas(modo) {
                    self.status = e;
                }
            }
            Comando::Lista(nome) => {
                if let Err(e) = self.sessao.lista_estilo(nome) {
                    self.status = e;
                }
            }
            Comando::Entrelinha(fator) => {
                if let Err(e) = self.sessao.entrelinha_definir(fator) {
                    self.status = e;
                }
            }
            Comando::Espaco(antes, pt) => {
                let f = &self.visao.formato;
                let (a, d) = if antes { (pt, f.depois) } else { (f.antes, pt) };
                if let Err(e) = self.sessao.espaco_definir(Some(a), Some(d)) {
                    self.status = e;
                }
            }
            Comando::Pincel => {
                if self.sessao.pincel_cheio() {
                    if !self.sessao.cursor().vazio() {
                        self.sessao.pincel_aplicar();
                        self.status = tr("Pincel aplicado à seleção").into();
                    } else {
                        self.status = tr("Pincel cancelado").into();
                    }
                    self.sessao.pincel_cancelar();
                } else {
                    self.sessao.pincel_copiar();
                    self.status = tr("Pincel ativo: selecione o destino com Shift+clique; Esc cancela.").into();
                }
            }
            Comando::Formato(bit) => {
                self.sessao.formatar(bit);
            }
            Comando::EstiloAtualizar | Comando::EstiloExcluir => {
                let id = self.sessao.estilo_com_nome_no_cursor();
                let nome = self.sessao.documento().estilos.pegar(id).map(|e| e.nome.clone());
                let r = match (nome, &c) {
                    (None, _) => Err(tr("O cursor não está num parágrafo com estilo com nome.").to_string()),
                    (Some(nome), Comando::EstiloAtualizar) => self.sessao
                        .estilo_modificar(id, crate::dialogos::estilo_do_cursor(&self.sessao, &nome)).map(|_| ()),
                    _ => self.sessao.estilo_excluir(id).map(|_| ()),
                };
                if let Err(e) = r {
                    self.status = e;
                }
            }
            Comando::EstiloNome(id) => {
                if let Err(e) = self.sessao.estilo_aplicar(id) {
                    self.status = e;
                }
            }
            Comando::Estilo(s) => {
                self.sessao.paragrafo_estilo(s);
            }
            Comando::Alinhar(s) => {
                if let Err(e) = self.sessao.alinhar(s) {
                    self.status = e;
                }
            }
            Comando::TabelaTam(l, c) => {
                if let Err(e) = self.sessao.inserir_tabela(l, c) {
                    self.status = e;
                }
            }
            Comando::Tabela => {
                if let Err(e) = self.sessao.inserir_tabela(3, 3) {
                    self.status = e;
                }
            }
            Comando::Grafico(tipo) => {
                if let Err(e) = featherdocs_io::grafico_docs::inserir_da_tabela(
                    &mut self.sessao, tipo,
                ) {
                    self.status = e;
                }
            }
            Comando::AtualizarGrafico => {
                if let Err(e) = featherdocs_io::grafico_docs::atualizar_no_cursor(
                    &mut self.sessao,
                ) {
                    self.status = e;
                }
            }
            Comando::TabelaEstrutura(acao) => {
                if let Err(erro) = featherdocs_tabelas::docs_editar_estrutura(
                    &mut self.sessao, acao,
                ) {
                    self.status = erro;
                }
            }
            Comando::TabelaMesclar => {
                if let Err(erro) = featherdocs_tabelas::docs_mesclar_celulas(&mut self.sessao) {
                    self.status = erro;
                }
            }
            Comando::TabelaDesmesclar => {
                if let Err(erro) = featherdocs_tabelas::docs_desmesclar_celulas(&mut self.sessao) {
                    self.status = erro;
                }
            }
            // Cabeçalho e exclusão continuam no Docs: são atributos do documento
            // e não dimensões/estilos genéricos do motor Tables.
            Comando::TabelaCabecalho => {
                if self.visao.formato.tabela.is_some() {
                    self.sessao.tabela_cabecalho();
                }
            }
            Comando::TabelaExcluir => {
                if let Err(erro) = self.sessao.tabela_excluir("tabela") {
                    self.status = erro;
                }
            }
            Comando::Imagem => {
                self.ocupado = true;
                return Task::perform(
                    async {
                        let Some(p) = dialogo_de_arquivo()
                            .add_filter("Imagens", &["png", "jpg", "jpeg"])
                            .pick_file()
                            .await
                        else {
                            return Ok(None);
                        };
                        let bytes = std::fs::read(p.path())
                            .map_err(|e| trf("Não consegui ler a imagem: {}", &[&e]))?;
                        let imagem = crate::imagem_compartilhada::preparar_insercao(&bytes)?;
                        Ok(Some(imagem))
                    },
                    Msg::Imagem,
                );
            }
            Comando::QuebraDeLinha => {
                self.sessao.inserir_texto(&featherdocs_core::modelo::QUEBRA_DE_LINHA.to_string());
            }
            Comando::EstiloRapido(k) => {
                if let Err(e) = crate::dialogos::aplicar_rapido(&mut self.sessao, k) {
                    self.status = e;
                }
            }
            Comando::Quebra => {
                self.sessao.inserir_bloco("quebra_de_pagina");
            }
            Comando::Sumario => {
                self.sessao.inserir_sumario(3);
            }
            Comando::Papel(p) => {
                if let Err(e) = self.sessao.papel_definir(p) {
                    self.status = e;
                }
            }
            Comando::Orientacao(b) => {
                if let Err(e) = self.sessao.orientacao_definir(b) {
                    self.status = e;
                }
            }
            Comando::Modo(m) => {
                if let Err(e) = self.sessao.modo_edicao_definir(m) {
                    self.status = e;
                }
            }
            Comando::PainelRevisao => self.painel_revisao = !self.painel_revisao,
            Comando::Comparar => {
                self.ocupado = true;
                return Task::perform(
                    async {
                        let Some(p) = dialogo_de_arquivo()
                            .add_filter("Documentos", &["fdoz", "docx", "md", "tex", "zip"])
                            .pick_file()
                            .await
                        else {
                            return Ok(None);
                        };
                        Self::carregar_em_background(p.path().to_path_buf()).await.map(Some)
                    },
                    Msg::Comparado,
                );
            }
            Comando::Revisoes(b, tudo) => {
                self.sessao.revisoes_resolver(b, tudo);
            }
            Comando::Estrutura => {
                self.config.estrutura = !self.config.estrutura;
                self.config.gravar();
            }
            Comando::Marcas => {
                self.sessao.marcas_definir(!self.sessao.estado().marcas);
            }
            Comando::Codigo(fonte) => self.alternar_codigo(fonte),
            Comando::Zoom(d) => {
                self.config.zoom = ((self.config.zoom + d) * 100.).round().clamp(50., 200.) / 100.;
                self.config.gravar();
            }
            Comando::Configuracoes => {
                self.menu_aberto = false;
                return self.alternar_aux(JanelaAux::Configuracoes);
            }
            Comando::Menu => {
                self.menu_aberto = !self.menu_aberto;
                self.inicio.pagina = crate::inicio::PaginaMenu::Inicio;
            }
            Comando::CampoParagrafo(qual, texto) => {
                if let Err(e) = faixa::aplicar_campo(self, qual, texto) {
                    self.status = e;
                }
            }
            Comando::Ajuda => {
                self.menu_aberto = false;
                return self.alternar_aux(JanelaAux::Ajuda);
            }
            Comando::Monitor => {
                self.monitor.amostrar();
                self.menu_aberto = false;
                return self.alternar_aux(JanelaAux::Monitor);
            }
            Comando::Buscar => {
                self.busca = !self.busca;
                self.foco_documento = !self.busca;
                if self.busca {
                    tarefa = Task::focus("busca");
                } else {
                    self.sessao.busca_limpar();
                }
            }
        }
        self.atualizar();
        tarefa
    }
    /// A língua do painel de código aberto (para a faixa marcar o botão).
    pub fn fonte_do_codigo(&self) -> Option<Fonte> {
        self.codigo.as_ref().map(|p| p.fonte)
    }

    /// Liga o painel nesta língua, troca de língua ou, na mesma, desliga.
    /// A vertente do documento: o .tex/.md aberto é código; o resto é o documento da página.
    fn vertente(&self) -> featherdocs_io::conversao::Vertente {
        match self.fonte_original.as_ref().map(|(f, _)| *f) {
            Some(Fonte::Latex) => featherdocs_io::conversao::Vertente::Latex,
            Some(Fonte::Markdown) => featherdocs_io::conversao::Vertente::Markdown,
            None => featherdocs_io::conversao::Vertente::Convencional,
        }
    }

    fn alternar_codigo(&mut self, fonte: Fonte) {
        self.aplicar_codigo();
        if self.fonte_do_codigo() == Some(fonte) {
            self.codigo = None;
            self.foco_documento = true;
            return;
        }
        // O Markdown não guarda layout, tabelas mescladas etc.: editar nele apagaria isso do documento.
        if fonte == Fonte::Markdown {
            let perdas = featherdocs_io::conversao::perdas(self.sessao.documento(), self.vertente(), "md");
            if !perdas.is_empty() {
                self.perdas_exportar = Some((None, perdas));
                return;
            }
        }
        self.mostrar_codigo(fonte);
    }

    fn mostrar_codigo(&mut self, fonte: Fonte) {
        let inicio = Instant::now();
        let mapa = painel_codigo::gerar(self.sessao.documento(), fonte, self.fonte_original.as_ref());
        let mut painel = PainelCodigo::novo(fonte, mapa, self.sessao.versao());
        painel.ir_ao_bloco(self.sessao.cursor().foco.bloco);
        self.codigo = Some(painel);
        self.status = trf("Código {} gerado em {} ms", &[&painel_codigo::nome_da_fonte(fonte), &inicio.elapsed().as_millis()]);
    }

    /// Manda ao motor o código digitado (se houver): relê, troca só os blocos que mudaram e
    /// guarda o texto como o original da pessoa. Cada aplicação é um passo de Desfazer.
    fn aplicar_codigo(&mut self) {
        let Some(painel) = self.codigo.as_mut() else { return };
        if painel.ultima_edicao.take().is_none() {
            return;
        }
        let fonte = painel.fonte;
        let texto = painel.conteudo.text();
        if self.sessao.modo_edicao() == "leitura" {
            self.status = tr("O documento está só para leitura: o código não foi aplicado.").into();
            painel.versao = u64::MAX;
            return;
        }
        let inicio = Instant::now();
        let versao_antes = self.sessao.versao();
        let r = featherdocs_io::dividido::reconciliar(self.sessao.documento(), fonte, &texto);
        if fonte == Fonte::Latex {
            self.avisos_latex = r.avisos.clone();
        }
        let mudanca = self.sessao.trocar_blocos(r.blocos, r.meta, r.numeracao);
        // O papel e as margens que o código escreve valem para a página (Layout mostra o mesmo).
        if let Some(p) = r.pagina.filter(|p| !featherdocs_io::dividido::mesma_pagina(p, &self.sessao.documento().pagina)) {
            let margens = |s: &mut Sessao| s.margens_definir(p.margem_esq, p.margem_dir, p.margem_sup, p.margem_inf);
            // A ordem importa: papel menor só cabe depois de margens menores, e o contrário.
            let ok = self.sessao.papel_personalizado(p.largura, p.altura).and_then(|_| margens(&mut self.sessao))
                .or_else(|_| margens(&mut self.sessao).and_then(|_| self.sessao.papel_personalizado(p.largura, p.altura)));
            if let Err(e) = ok {
                self.status = e;
            }
        }
        if let Some(e) = r.extras {
            self.sessao.mudar_extras(|x| *x = e);
        }
        painel.blocos = featherdocs_io::exportar::mapa_de_texto(&texto, self.sessao.documento(), fonte).blocos;
        painel.versao = self.sessao.versao();
        painel.bloco_marcado = painel.bloco_da_linha(painel.linha_do_cursor());
        self.status = if self.sessao.versao() == versao_antes {
            String::new()
        } else {
            trf("Código {} aplicado: {} bloco(s) em {} ms",
                &[&painel_codigo::nome_da_fonte(fonte), &mudanca.afetados.len(), &inicio.elapsed().as_millis()])
        };
        self.fonte_original = Some((fonte, texto));
    }

    /// Depois de mudar a sessão: se o documento mudou por fora do código (página, faixa,
    /// desfazer), o código é gerado de novo; se a pessoa está na página, o código acompanha
    /// o bloco do cursor. Nada disso enquanto há código digitado à espera da pausa.
    fn sincronizar_codigo(&mut self) {
        let versao = self.sessao.versao();
        let bloco = self.sessao.cursor().foco.bloco;
        let Some(painel) = self.codigo.as_mut() else { return };
        if painel.pendente() {
            return;
        }
        if painel.versao != versao {
            let mapa = painel_codigo::gerar(self.sessao.documento(), painel.fonte, self.fonte_original.as_ref());
            painel.recarregar(mapa, versao);
        }
        if self.foco_documento {
            painel.ir_ao_bloco(bloco);
        }
    }

    /// Uma tecla ou um clique terminou com o código em foco: se o cursor do código foi a outra
    /// linha, a página vai ao bloco dela (o Iced mandava cada movimento como `Action`).
    fn codigo_cursor(&mut self) -> Task<Msg> {
        if self.foco_documento || self.ocupado || self.pendente.is_some() {
            return Task::none();
        }
        let Some(painel) = self.codigo.as_mut() else { return Task::none() };
        if painel.pendente() {
            return Task::none();
        }
        let Some(id) = painel.bloco_da_linha(painel.linha_do_cursor()) else { return Task::none() };
        if painel.bloco_marcado == Some(id) {
            return Task::none();
        }
        painel.bloco_marcado = Some(id);
        if self.sessao.cursor().foco.bloco != id {
            self.sessao.ir_para_bloco(id);
            self.atualizar();
            return self.rolar_ao_cursor();
        }
        Task::none()
    }

    /// `FEATHERDOCS_CAPTURAR=arquivo.png`: espera o arquivo da linha de comando abrir, liga o
    /// painel de código (`FEATHERDOCS_CAPTURAR_CODIGO=tex|md`), fotografa a janela e fecha.
    /// `FEATHERDOCS_CAPTURAR_TEMA=escuro|claro` escolhe o tema só para a foto.
    fn capturar_tela(&mut self) -> Task<Msg> {
        if self.ocupado {
            return Task::none();
        }
        match self.passo_captura {
            0 => {
                if let Ok(tema) = std::env::var("FEATHERDOCS_CAPTURAR_TEMA") {
                    definir_tema(tema == "escuro", self.config.acento);
                }
                // `FEATHERDOCS_CAPTURAR_INICIO=1`: a tela inicial; `FEATHERDOCS_CAPTURAR_MENU=inicio|recentes|exportar|informacoes`: o menu Arquivos.
                self.inicio.tela_cheia = std::env::var_os("FEATHERDOCS_CAPTURAR_INICIO").is_some();
                if let Ok(m) = std::env::var("FEATHERDOCS_CAPTURAR_MENU") {
                    use crate::inicio::PaginaMenu;
                    self.menu_aberto = true;
                    self.inicio.pagina = match m.as_str() {
                        "recentes" => PaginaMenu::Recentes,
                        "exportar" => PaginaMenu::Exportar,
                        "informacoes" => PaginaMenu::Informacoes,
                        _ => PaginaMenu::Inicio,
                    };
                }
                match std::env::var("FEATHERDOCS_CAPTURAR_CODIGO").as_deref() {
                    Ok("tex") => self.alternar_codigo(Fonte::Latex),
                    Ok("md") => self.alternar_codigo(Fonte::Markdown),
                    _ => {}
                }
                // `FEATHERDOCS_CAPTURAR_ZOOM=1.5`: o zoom só para a foto (não grava na configuração).
                if let Some(z) = std::env::var("FEATHERDOCS_CAPTURAR_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()) {
                    self.config.zoom = z.clamp(0.5, 2.);
                }
                if let Ok(n) = std::env::var("FEATHERDOCS_CAPTURAR_BLOCO") {
                    // Leva a página (e o código) ao n-ésimo bloco do documento.
                    if let Some(id) = n.parse::<usize>().ok()
                        .and_then(|i| self.sessao.documento().blocos.get(i)).map(|b| b.id) {
                        self.sessao.ir_para_bloco(id);
                    }
                }
                self.aba = match std::env::var("FEATHERDOCS_CAPTURAR_ABA").as_deref() {
                    Ok("inicio") => Aba::Inicio,
                    Ok("inserir") => Aba::Inserir,
                    Ok("layout") => Aba::Layout,
                    Ok("revisao") => Aba::Revisao,
                    Ok("desenhar") => Aba::Desenhar,
                    Ok("design") => Aba::Design,
                    Ok("referencias") => Aba::Referencias,
                    Ok("correspondencias") => Aba::Correspondencias,
                    _ => Aba::Exibir,
                };
                self.painel_revisao |= self.aba == Aba::Revisao;
                // `FEATHERDOCS_CAPTURAR_PENDENTE=1`: a pergunta de salvar antes de um documento novo.
                if std::env::var_os("FEATHERDOCS_CAPTURAR_PENDENTE").is_some() {
                    self.pendente = Some(Destino::Novo);
                }
                // `FEATHERDOCS_CAPTURAR_BUSCA=texto`: o painel de busca aberto, com Substituir.
                if let Ok(t) = std::env::var("FEATHERDOCS_CAPTURAR_BUSCA") {
                    self.busca = true;
                    self.substituir_visivel = true;
                    self.busca_opcoes.palavra = true;
                    let _ = self.update(Msg::Consulta(t));
                }
                // `FEATHERDOCS_CAPTURAR_ESTILOS=x`: a galeria de estilos aberta, como se o botão estivesse em x.
                if let Some(x) = std::env::var("FEATHERDOCS_CAPTURAR_ESTILOS").ok().and_then(|x| x.parse().ok()) {
                    self.ponto_menu_faixa = Point::new(x, 0.);
                    self.menu_faixa = Some(faixa::MenuFaixa::Estilos);
                }
                // `FEATHERDOCS_CAPTURAR_RECOLHIDA=3` (ou `3,0`): faixa recolhida com esse menu clássico
                // (e o submenu) aberto; `-` só recolhe.
                if let Ok(v) = std::env::var("FEATHERDOCS_CAPTURAR_RECOLHIDA") {
                    self.config.faixa_recolhida = true;
                    let mut n = v.split(',').filter_map(|x| x.parse::<u8>().ok());
                    self.menu_faixa = n.next().map(|m| faixa::MenuFaixa::Classico(m, n.next()));
                }
                // `FEATHERDOCS_CAPTURAR_MENU_FAIXA=margens@300`: um menu da faixa aberto, descendo do x 300
                // (`grupo1` abre o grupo recolhido 1).
                if let Ok(v) = std::env::var("FEATHERDOCS_CAPTURAR_MENU_FAIXA") {
                    use faixa::MenuFaixa as M;
                    let (nome, x) = v.split_once('@').unwrap_or((&v, "200"));
                    self.ponto_menu_faixa = Point::new(x.parse().unwrap_or(200.), 0.);
                    self.menu_faixa = match nome {
                        "margens" => Some(M::Margens),
                        "orientacao" => Some(M::Orientacao),
                        "tamanho" => Some(M::Tamanho),
                        "numeros" => Some(M::NumerosLinha),
                        "vertical" => Some(M::Vertical),
                        "graficos" => Some(M::Graficos),
                        "quebras" => Some(M::Quebras),
                        "marca" => Some(M::MarcaDagua),
                        g => g.strip_prefix("grupo").and_then(|i| i.parse().ok()).map(M::Grupo),
                    };
                }
                // `FEATHERDOCS_CAPTURAR_TINTA=1`: traços de exemplo na página 1 e a Caneta 2 escolhida;
                // `menu` deixa aberto o menu de cor e espessura dela.
                if let Ok(v) = std::env::var("FEATHERDOCS_CAPTURAR_TINTA") {
                    self.preparar_captura_tinta(v == "menu");
                }
                // `FEATHERDOCS_CAPTURAR_EXPORTAR_PDF=1`: a janela Exportar PDF.
                if std::env::var_os("FEATHERDOCS_CAPTURAR_EXPORTAR_PDF").is_some() {
                    let _ = self.comando(Comando::ExportarPdf);
                }
                // `FEATHERDOCS_CAPTURAR_LINHA=26`: a aba Avisos e o código LaTeX naquela linha.
                if let Some(n) = std::env::var("FEATHERDOCS_CAPTURAR_LINHA").ok().and_then(|n| n.parse().ok()) {
                    self.lateral = 2;
                    let _ = self.update(Msg::IrLinhaCodigo(n));
                }
                // `FEATHERDOCS_CAPTURAR_ARQUIVO=Referencias.bib`: abre um arquivo do projeto LaTeX.
                if let Ok(nome) = std::env::var("FEATHERDOCS_CAPTURAR_ARQUIVO") {
                    self.lateral = 1;
                    let _ = self.update(Msg::AbrirArquivo(nome));
                }
                self.atualizar();
                // `FEATHERDOCS_CAPTURAR_PDF=saida.pdf`: grava também o PDF das páginas.
                if let Some(saida) = std::env::var_os("FEATHERDOCS_CAPTURAR_PDF") {
                    let meta = &self.sessao.documento().meta;
                    let opcoes = crate::pdf::OpcoesPdf::do_documento(&meta.titulo, &meta.autor);
                    match crate::pdf::gerar_com(&self.quadros_impressos(), &opcoes, &self.sessao.links_do_pdf(), &self.sessao.marcadores_do_pdf()) {
                        Ok(b) => { let _ = std::fs::write(&saida, b); }
                        Err(e) => eprintln!("PDF não gerado: {e}"),
                    }
                }
                self.passo_captura = 1;
                // Maximizada, a faixa aparece inteira na foto (a janela nasce com 1200 px).
                let maximizar = match self.janelas.principal {
                    Some(id) => Task::window_of(id, WindowCmd::Maximize(true)),
                    None => Task::none(),
                };
                Task::batch([maximizar, self.rolar_ao_cursor()])
            }
            1 => {
                self.passo_captura = 2;
                // A foto sai no pulso seguinte (1,5 s depois), com a janela já maximizada: o pelicano
                // desenha a mesma árvore direto num PNG, no tamanho da janela.
                // shortcut: a foto monta a árvore com estado novo, então sai sem a rolagem da página e do
                // código (o `rolar_ao_cursor` acima não vale nela); fotografar o quadro da janela pede API no pelicano.
                let Some(id) = self.janelas.principal else { return Task::none() };
                // `FEATHERDOCS_CAPTURAR_LARGURA=900`: a foto como numa janela estreita (a faixa encolhe).
                if let Some(l) = std::env::var("FEATHERDOCS_CAPTURAR_LARGURA").ok().and_then(|l| l.parse::<f32>().ok()) {
                    self.tamanho_principal.width = l.max(320.);
                }
                // `FEATHERDOCS_CAPTURAR_JANELA=primeiros:<passo 1–4>|novidades[:<versão>]`: a janela, no tamanho dela.
                let janela = std::env::var("FEATHERDOCS_CAPTURAR_JANELA").ok();
                let (nome, n) = janela.as_deref().map_or(("", 1), |j| j.split_once(':').map_or((j, 1), |(a, b)| (a, b.parse().unwrap_or(1))));
                let (arvore, tamanho) = match nome {
                    "primeiros" => {
                        self.passo = n.max(1) - 1;
                        (self.view_primeiros_passos(), Size::new(1000.0, 680.0))
                    }
                    "novidades" => {
                        self.novidades_versao = n.max(1) - 1;
                        (self.view_novidades(), Size::new(1000.0, 680.0))
                    }
                    _ => (self.view(id), self.tamanho_principal),
                };
                let png = pelicano::capture(&arvore, tamanho, 1.0, &self.tema(id), &mut pelicano::Renderer::new(), None);
                if let Some(caminho) = std::env::var_os("FEATHERDOCS_CAPTURAR") {
                    if let Err(e) = std::fs::write(&caminho, png) {
                        eprintln!("Captura não gravada: {e}");
                    }
                }
                Task::window(WindowCmd::Exit)
            }
            _ => Task::none(),
        }
    }

    /// Um arquivo do projeto LaTeX, só para ver: a imagem inteira ou o texto com os números de linha.
    fn view_arquivo<'a>(&'a self, nome: &'a str, visto: &'a Visto) -> Element<'a, Msg> {
        let zoom: Element<'a, Msg> = if matches!(visto, Visto::Imagem(..)) {
            let botao = |ic: &'static str, m: Msg| button(icone(ic, 14.)).padding(4).style(estilo_botao_pequeno(false)).on_press(m);
            row![
                botao("zoom-out", Msg::ZoomArquivo(1. / 1.25)),
                button(text(format!("{:.0}%", self.zoom_arquivo * 100.)).size(11)).padding([3u16, 6]).style(estilo_botao_pequeno(false)).on_press(Msg::ZoomArquivo(0.)),
                botao("zoom-in", Msg::ZoomArquivo(1.25)),
            ]
            .spacing(2)
            .align(Align::Center)
            .into()
        } else {
            text(tr("Somente leitura")).size(11).color(COR_TEXTO_MUTED()).into()
        };
        let topo = container(
            row![
                icone(if matches!(visto, Visto::Imagem(..)) { "photo" } else { "file-code" }, 16.),
                text(nome).size(13),
                horizontal_space(),
                zoom,
                button(icone("x", 14.)).padding(4u16).style(estilo_botao_pequeno(false)).on_press(Msg::FecharArquivo),
            ]
            .spacing(8)
            .align(Align::Center),
        )
        .padding([6u16, 10])
        .width(Length::FILL)
        .style(estilo_container_topo);
        let corpo: Element<'a, Msg> = match visto {
            // 100% = a imagem inteira cabendo no painel (o `responsive` dá o espaço); maior, rola nas barras.
            Visto::Imagem(h, w, a) => {
                let (h, w, a, z) = (h.clone(), *w as f32, *a as f32, self.zoom_arquivo);
                pelicano::responsive(move |espaco| {
                    let ajuste = ((espaco.width - 24.) / w).min((espaco.height - 24.) / a).max(0.01);
                    let imagem = pelicano::image(h.clone()).width(w * ajuste * z).height(a * ajuste * z);
                    scrollable(container(imagem).padding(12u16))
                        .direction(pelicano::Direction::Both)
                        .width(Length::FILL)
                        .height(Length::FILL)
                        .into()
                })
                .into()
            }
            Visto::Texto(t) => {
                let numeros = (1..=t.lines().count().max(1)).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
                let mono = |s: String| text(s).font(painel_codigo::FONTE_MONO).size(painel_codigo::TAMANHO);
                scrollable(
                    row![
                        container(mono(numeros).color(COR_TEXTO_MUTED()).align_x(Align::End))
                            .padding(Padding { top: painel_codigo::PADDING, bottom: painel_codigo::PADDING, left: 8., right: 6. }),
                        container(mono(t.clone()).color(COR_TEXTO()).no_wrap()).padding(painel_codigo::PADDING),
                    ],
                )
                .direction(pelicano::Direction::Both)
                .height(Length::FILL)
                .into()
            }
            Visto::Aviso(a) => container(text(a.as_str()).size(13).color(COR_TEXTO_MUTED())).padding(20u16).into(),
        };
        container(column![topo, corpo])
            .width(Length::Fill(45))
            .height(Length::FILL)
            .style(|_t| Style::default().background(COR_PAINEL()))
            .into()
    }

    fn view_codigo<'a>(&'a self, painel: &'a PainelCodigo) -> Element<'a, Msg> {
        let (linha, coluna) = painel.conteudo.cursor();
        let (titulo, outra, rotulo_outra) = match painel.fonte {
            Fonte::Latex => (tr("codigo_titulo_latex"), Fonte::Markdown, tr("codigo_markdown")),
            Fonte::Markdown => (tr("codigo_titulo_markdown"), Fonte::Latex, tr("codigo_latex")),
        };
        let topo = container(
            row![
                icone("code", 16.),
                text(titulo).size(13),
                horizontal_space(),
                text(trf("Lin {}, Col {}", &[&linha, &coluna]))
                    .size(11)
                    .color(COR_TEXTO_MUTED()),
                button(text(rotulo_outra).size(12))
                    .padding([3u16, 9])
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::Comando(Comando::Codigo(outra))),
                button(icone("x", 14.))
                    .padding(4u16)
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::CodigoFechar),
            ]
            .spacing(8)
            .align(Align::Center),
        )
        .padding([6u16, 10])
        .width(Length::FILL)
        .style(estilo_container_topo);
        let f = usize::from(painel.fonte == Fonte::Markdown);
        // O realce lê o tema de uma variável global: vale o do painel aberto (só há um).
        painel_codigo::definir_tema(self.config.tema_codigo[f]);
        // A linha com algo que não foi convertido fica marcada (a lista está em Avisos).
        // shortcut: o pelicano pinta os números de linha ele mesmo, sem cor por linha: a linha com aviso
        // vai sublinhada em onda com a dica, em vez do número na cor de erro.
        let avisos = if painel.fonte == Fonte::Latex {
            self.avisos_latex.iter().flat_map(|a| a.linhas.iter().map(move |&l| (l, a))).filter_map(|(l, a)| {
                let texto = painel.conteudo.line(l)?;
                Some(pelicano::Warning { line: l - 1, range: 0..texto.len(), tip: tr(a.tipo.rotulo()).to_string() })
            }).collect()
        } else {
            Vec::new()
        };
        let editor = pelicano::text_editor(&painel.conteudo)
            .on_change(Msg::CodigoMudou)
            .font(painel_codigo::FONTE_MONO)
            .size(painel_codigo::TAMANHO)
            .line_height(painel_codigo::ALTURA_DA_LINHA)
            .padding(painel_codigo::PADDING)
            .wrap(false)
            // Só escreve depois de clicado: sem foco, as teclas vão para a página.
            .idle_focus(false)
            .line_numbers(self.config.numeros_de_linha[f])
            .warnings(avisos)
            .highlight(if painel.fonte == Fonte::Latex { "tex" } else { "md" }, painel_codigo::destaque(painel_codigo::tema()))
            .on_key(|tecla, mods| {
                // Ctrl+Z/Ctrl+Y no código desfazem o documento (cada aplicação do código é um passo).
                let keyboard::Key::Character(c) = tecla else { return None };
                match (c.to_lowercase().as_str(), mods.command(), mods.shift()) {
                    ("z", true, false) => Some(Msg::Comando(Comando::Desfazer)),
                    ("z", true, true) | ("y", true, _) => Some(Msg::Comando(Comando::Refazer)),
                    _ => None,
                }
            })
            .style(|_tema, status| Style::default()
                .background(COR_PAINEL())
                .text_color(COR_TEXTO())
                .border(Border::rounded(0.).color(if status == Status::Focused { COR_AZUL_ACENTO() } else { COR_BORDA() }, 0.)));
        let corpo = crate::compat::ao_clicar(editor, Msg::CodigoFoco);
        container(column![topo, corpo])
            .width(Length::Fill(45))
            .height(Length::FILL)
            .style(|_t| Style::default().background(COR_PAINEL()).border(Border::rounded(0.).color(COR_BORDA(), 1.)))
            .into()
    }
    fn rolar_ao_cursor(&self) -> Task<Msg> {
        let Some(c) = self.sessao.geometria(&self.sessao.cursor().foco) else {
            return Task::none();
        };
        let y = self
            .quadros
            .iter()
            .take(c.pagina)
            .map(|q| q.altura * self.config.zoom + 24.)
            .sum::<f32>()
            + c.y * self.config.zoom;
        Task::scroll_to("documento", Point::new(0., (y - 140.).max(0.)))
    }
    pub fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            if self.codigo.as_ref().is_some_and(PainelCodigo::pendente) {
                Subscription::every(std::time::Duration::from_millis(100)).map(|_| Msg::CodigoPulso)
            } else {
                Subscription::none()
            },
            if self.janelas.aberta(JanelaAux::Monitor) {
                Subscription::every(std::time::Duration::from_secs(1)).map(|_| Msg::MonitorPulso)
            } else {
                Subscription::none()
            },
            if self.passo_captura < 2 && std::env::var_os("FEATHERDOCS_CAPTURAR").is_some() {
                Subscription::every(std::time::Duration::from_millis(1500)).map(|_| Msg::CapturarTela)
            } else {
                Subscription::none()
            },
        ])
    }
    /// Os eventos de cada janela (o `listen_with`, `close_requests` e `close_events` do Iced).
    /// `livre`: nenhum widget usou o evento (o `event::Status::Ignored` do Iced).
    fn evento_janela(&self, id: WindowId, ev: &Event, livre: bool) -> Option<Msg> {
        match ev {
            Event::CursorMoved(p) if self.arrastando_estrutura => Some(Msg::EstruturaMover(p.x)),
            Event::MouseReleased(_) if self.arrastando_estrutura => Some(Msg::EstruturaSoltar),
            Event::Key { key, modifiers, pressed: true, .. } if livre => Some(Msg::AtalhoJanela(id, key.clone(), *modifiers)),
            // A tecla ou o clique já passou pelo editor de código: o cursor dele pode ter mudado de bloco.
            Event::Key { pressed: true, .. } | Event::MouseReleased(_) if self.codigo.is_some() && !self.foco_documento => Some(Msg::CodigoCursor),
            Event::FileDropped(p) => Some(Msg::AbrirRecente(p.clone())),
            Event::CloseRequested => Some(Msg::Fechar(id)),
            Event::Resized(t) if Some(id) == self.janelas.principal => Some(Msg::JanelaRedimensionada(*t)),
            _ => None,
        }
    }
    pub fn view(&self, janela: window::Id) -> Element<'_, Msg> {
        feather_ui::cromo::moldura_janela(self.view_conteudo(janela))
    }

    fn view_conteudo(&self, janela: window::Id) -> Element<'_, Msg> {
        let auxiliar = self.janelas.tipo(janela);
        if let Some(tipo) = auxiliar {
            let corpo = match tipo {
                JanelaAux::Configuracoes => self.view_configuracoes(),
                JanelaAux::Ajuda => self.view_ajuda(),
                JanelaAux::Monitor => self.view_monitor(),
                JanelaAux::PrimeirosPassos => self.view_primeiros_passos(),
                JanelaAux::Novidades => self.view_novidades(),
            };
            if self.config.barra_propria {
                return feather_ui::cromo::janela_com_barra(
                    &self.titulo(janela), corpo, Self::acoes_janela(janela)
                );
            }
            return corpo;
        }
        if self.inicio.tela_cheia {
            let corpo = self.view_tela_inicio();
            if self.config.barra_propria {
                return feather_ui::cromo::janela_com_barra(&self.titulo(janela), corpo, Self::acoes_janela(janela));
            }
            return corpo;
        }
        self.view_editor(janela)
    }
    fn view_editor(&self, janela: window::Id) -> Element<'_, Msg> {
        let titulo_documento = self.caminho.as_ref()
            .and_then(|p| p.file_name()).and_then(|n| n.to_str())
            .unwrap_or(tr("Documento sem título"));
        let topo = feather_ui::cromo::barra_editor(
            titulo_documento,
            self.sessao.estado().sujo,
            Some(Msg::Comando(Comando::Menu)),
            [
                feather_ui::cromo::Acao { icone: "file-plus", dica: tr("Novo (Ctrl+N)"), mensagem: Some(Msg::Comando(Comando::Novo)) },
                feather_ui::cromo::Acao { icone: "folder-open", dica: tr("Abrir (Ctrl+O)"), mensagem: Some(Msg::Comando(Comando::Abrir)) },
                feather_ui::cromo::Acao { icone: "device-floppy", dica: tr("Salvar (Ctrl+S)"), mensagem: Some(Msg::Comando(Comando::Salvar)) },
                feather_ui::cromo::Acao { icone: "file-export", dica: tr("Exportar"), mensagem: Some(Msg::Comando(Comando::Exportar)) },
                feather_ui::cromo::Acao { icone: "arrow-back-up", dica: tr("Desfazer (Ctrl+Z)"), mensagem: Some(Msg::Comando(Comando::Desfazer)) },
                feather_ui::cromo::Acao { icone: "arrow-forward-up", dica: tr("Refazer"), mensagem: Some(Msg::Comando(Comando::Refazer)) },
                feather_ui::cromo::Acao { icone: "search", dica: tr("Pesquisar (Ctrl+F)"), mensagem: Some(Msg::Comando(Comando::Buscar)) },
                feather_ui::cromo::Acao { icone: "help", dica: tr("Ajuda (F1)"), mensagem: Some(Msg::Comando(Comando::Ajuda)) },
                feather_ui::cromo::Acao { icone: "device-desktop", dica: tr("Monitor (F12)"), mensagem: Some(Msg::Comando(Comando::Monitor)) },
            ],
            Msg::Comando(Comando::Configuracoes),
        );
        let topo = if self.config.barra_propria {
            feather_ui::cromo::barra_integrada(topo, Self::acoes_janela(janela))
        } else { topo };
        let folhas = move || {
            let mut folhas = column![].spacing(24).align(Align::Center);
            for (i, q) in self.quadros.iter().enumerate() {
                let pagina: Element<'_, Msg> = canvas(Pagina {
                        quadro: q,
                        visao: &self.visao,
                        numero: i,
                        escala: self.config.zoom,
                        ativa: self.foco_documento
                            && self.pagina_ativa == i
                            && !self.ocupado
                            && self.pendente.is_none(),
                        imagens: &self.imagens,
                        arrastando: self.arrastando,
                    })
                    .width(q.largura * self.config.zoom)
                    .height(q.altura * self.config.zoom)
                    .into();
                // A tinta da aba Desenhar por cima da página.
                folhas = folhas.push(self.pagina_com_tinta(i, pagina));
            }
            folhas
        };
        // O Iced centrava as páginas na vista; aqui a largura da vista vem do `responsive` e a coluna
        // ocupa pelo menos ela (com a página mais larga que a vista, rola na horizontal).
        let largura = self.quadros.iter().map(|q| q.largura * self.config.zoom).fold(0., f32::max) + 48.;
        let paginas = container(pelicano::responsive(move |espaco| {
            scrollable(
                container(folhas())
                    .padding(24u16)
                    .width(espaco.width.max(largura))
                    .align_x(Align::Center),
            )
            .direction(pelicano::Direction::Both)
            .id("documento")
            .on_scroll(|v| {
                let o = v.offset;
                Msg::Rolou(Vector::new(o.x, o.y))
            })
            .height(Length::FILL)
            .into()
        }))
        .height(Length::FILL)
        .width(if self.codigo.is_some() || self.arquivo_visto.is_some() { Length::Fill(55) } else { Length::FILL });
        let mut centro = row![].height(Length::FILL);
        // A alça da borda direita do painel Estrutura (ou da beirada esquerda, com ele fechado no meio do arrasto).
        let alca = || -> Element<'_, Msg> {
            let ativa = self.arrastando_estrutura;
            pelicano::mouse_area(
                container(Space::new(Length::Fixed(5.0), Length::FILL)).style(move |_| Style {
                    background: ativa.then(|| pelicano::Background::Color(COR_AZUL_ACENTO().alpha(0.6))),
                    ..Default::default()
                }),
            )
            .on_press(Msg::EstruturaArrastar)
            .cursor_icon(pelicano::CursorIcon::EwResize)
            .into()
        };
        if !self.config.estrutura && self.arrastando_estrutura {
            centro = centro.push(alca());
        }
        if self.config.estrutura {
            let mut titulos = column![].spacing(7);
            // Num projeto LaTeX o topo escolhe a lista: 0 estrutura, 1 arquivos, 2 o que não foi convertido.
            let lateral = if self.arquivos_projeto.is_empty() { 0 } else { self.lateral };
            if !self.arquivos_projeto.is_empty() {
                let aba = |rotulo: String, v: u8| {
                    button(text(rotulo).size(11.5)).padding([5, 4]).width(Length::FILL)
                        .style(estilo_botao_pequeno(lateral == v)).on_press(Msg::Lateral(v))
                };
                titulos = titulos.push(row![
                    aba(tr("estrutura").into(), 0),
                    aba(tr("Arquivos").into(), 1),
                    aba(trf("Avisos ({})", &[&self.avisos_latex.len()]), 2),
                ].spacing(3));
            }
            let arquivos = lateral == 1;
            // Como a árvore do Overleaf: o .tex abre o código; o resto abre para ver (só leitura).
            if arquivos {
                for nome in &self.arquivos_projeto {
                    let tex = nome.to_ascii_lowercase().ends_with(".tex");
                    let imagem = ["png", "jpg", "jpeg", "gif", "svg", "eps", "pdf"].iter().any(|x| nome.to_ascii_lowercase().ends_with(&format!(".{x}")));
                    let linha = row![icone(if imagem { "photo" } else { "file-code" }, 14.), text(nome.as_str()).size(12)].spacing(6).align(Align::Center);
                    let ativo = match &self.arquivo_visto {
                        Some((visto, _)) => visto == nome,
                        None => tex && self.fonte_do_codigo() == Some(Fonte::Latex),
                    };
                    titulos = titulos.push(button(linha).padding([5, 10]).width(Length::FILL).style(estilo_botao_pequeno(ativo)).on_press(Msg::AbrirArquivo(nome.clone())));
                }
            }
            // O que o leitor não converteu; o clique leva à linha no código LaTeX.
            if lateral == 2 {
                if self.avisos_latex.is_empty() {
                    titulos = titulos.push(text(tr("Tudo do código LaTeX foi convertido.")).size(12).color(COR_TEXTO_MUTED()));
                }
                for a in &self.avisos_latex {
                    let mut detalhe = tr(a.tipo.rotulo()).to_string();
                    if a.linha > 0 {
                        detalhe = trf("{} · linha {}", &[&detalhe, &a.linha]);
                    }
                    if a.vezes > 1 {
                        detalhe = trf("{} · {} vezes", &[&detalhe, &a.vezes]);
                    }
                    let item = row![
                        icone("alert-triangle", 14.),
                        column![text(a.nome.as_str()).font(painel_codigo::FONTE_MONO).size(12), text(detalhe).size(11).color(COR_TEXTO_MUTED())].spacing(2),
                    ]
                    .spacing(6)
                    .align(Align::Center);
                    let mut b = button(item).padding([5, 8]).width(Length::FILL).style(estilo_botao_pequeno(false));
                    if a.linha > 0 {
                        b = b.on_press(Msg::IrLinhaCodigo(a.linha));
                    }
                    titulos = titulos.push(b);
                }
            }
            let estrutura = if lateral != 0 { Vec::new() } else { self.sessao.estrutura() };
            if self.arquivos_projeto.is_empty() {
                titulos = titulos.push(text(tr("estrutura")).size(16));
            }
            if estrutura.is_empty() && lateral == 0 {
                titulos = titulos.push(text(tr("sem_titulos")).size(12).color(COR_TEXTO_MUTED()));
            }
            for t in estrutura {
                titulos = titulos.push(
                    button(text(t.texto).size(12))
                        .padding([6, 10 + (t.nivel.saturating_sub(1) as u16 * 10)])
                        .width(Length::FILL)
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::IrBloco(t.id)),
                );
            }
            centro = centro.push(
                container(scrollable(titulos))
                    .padding(14)
                    .width(self.config.largura_estrutura)
                    .height(Length::FILL)
                    .style(estilo_painel_miniaturas),
            );
            centro = centro.push(alca());
        }
        if let Some((nome, visto)) = &self.arquivo_visto {
            centro = centro.push(self.view_arquivo(nome, visto));
        } else if let Some(painel) = &self.codigo {
            centro = centro.push(self.view_codigo(painel));
        }
        centro = centro.push(paginas);
        if self.painel_revisao {
            centro = centro.push(self.view_revisao());
        }
        let e = self.sessao.estado();
        let rodape = container(
            row![
                text(format!(
                    "{} {} / {} · {} {}",
                    tr("pagina"),
                    self.pagina_ativa + 1,
                    e.paginas,
                    e.palavras,
                    tr("palavras")
                ))
                .size(12),
                text(if self.ocupado {
                    tr("ocupado")
                } else {
                    &self.status
                })
                .size(12),
                horizontal_space(),
                text(format!("{:.0}%", self.config.zoom * 100.)).size(12),
                text(tr("local")).size(11)
            ]
            .spacing(16),
        )
        .padding([7, 12])
        .style(estilo_container_topo);
        let mut tela = column![].spacing(0);
        // Recolhida, a faixa vira a barra dos menus clássicos (`menu_classico`).
        tela = tela.push(
            crate::compat::ao_mover_relativo(container(faixa::view(self)).style(estilo_painel_faixa), Msg::PonteiroFaixa),
        );
        if let Some((_, _, chave)) = &self.efeito_imagem {
            tela = tela.push(
                container(row![
                    horizontal_space(),
                    feather_ui::efeitos_imagem::painel(
                        chave,
                        Msg::AplicarEfeitoImagem,
                        Msg::FecharEfeitosImagem,
                    ),
                ]).width(Length::FILL).padding(8)
            );
        }
        if self.busca {
            tela = tela.push(self.view_busca());
        }
        if let Some((p, perdas)) = &self.perdas_exportar {
            tela = tela.push(self.view_perdas(p.as_deref(), perdas));
        } else {
            tela = tela.push(centro);
        }
        let corpo: Element<'_, Msg> = tela.push(rodape).height(Length::FILL).into();
        // Fechada, cada camada põe a tela numa pilha de um item só (`so`).
        let corpo = match self.menu_faixa {
            Some(m) => self.view_menu_faixa(corpo, m),
            None => so(corpo),
        };
        let corpo = self.sobrepor_menu_tinta(corpo);
        let tela: Element<'_, Msg> = column![topo, corpo].spacing(0).into();
        let tela = if self.menu_aberto { self.sobrepor_backstage(tela) } else { so(tela) };
        let tela = match self.menu_contexto {
            Some(ponto) => self.view_contexto(tela, ponto),
            None => so(tela),
        };
        let tela = match &self.formulario {
            Some(f) => f.view(&self.sessao, tela),
            None => so(tela),
        };
        let tela = match &self.opcoes_pdf {
            Some(o) => self.view_exportar_pdf(o, tela),
            None => so(tela),
        };
        match &self.pendente {
            Some(destino) => self.view_salvar_pendente(destino, tela),
            None => so(tela),
        }
    }

    /// Exportar PDF com as opções da janela: escolhe o arquivo, grava e (se pedido) abre no leitor do sistema.
    fn exportar_pdf(&mut self, opcoes: crate::pdf::OpcoesPdf) -> Task<Msg> {
        if let Err(e) = crate::pdf::paginas_do_intervalo(&opcoes.paginas, self.quadros.len()) {
            self.status = e;
            self.opcoes_pdf = Some(opcoes);
            return Task::none();
        }
        if self.ocupado {
            return Task::none();
        }
        self.ocupado = true;
        let quadros = self.quadros_impressos();
        let (links, marcadores) = (self.sessao.links_do_pdf(), self.sessao.marcadores_do_pdf());
        let nome = self.caminho.as_ref().and_then(|c| c.file_stem()).map_or_else(|| "documento".to_string(), |n| n.to_string_lossy().into_owned());
        let versao = self.sessao.versao();
        Task::perform(
            async move {
                let Some(f) = dialogo_de_arquivo().set_file_name(format!("{nome}.pdf")).add_filter("PDF", &["pdf"]).save_file().await else {
                    return Ok(None);
                };
                let mut p = f.path().to_path_buf();
                if p.extension().is_none() {
                    p.set_extension("pdf");
                }
                tokio::task::spawn_blocking(move || {
                    let b = crate::pdf::gerar_com(&quadros, &opcoes, &links, &marcadores)?;
                    arquivos::gravar(&p, &b)?;
                    if opcoes.abrir_depois {
                        let _ = abrir_no_sistema(p.as_os_str());
                    }
                    Ok::<_, String>(Some((p, versao, true)))
                })
                .await
                .map_err(|e| trf("Falha ao salvar documento: {}", &[&e]))?
            },
            Msg::Gravado,
        )
    }

    /// Exportar PDF por cima do documento: páginas, marca d'água, dados do documento e o que o
    /// PDF leva (links, marcadores, fontes da tela), tudo já preenchido com o comum.
    fn view_exportar_pdf<'a>(&'a self, o: &'a crate::pdf::OpcoesPdf, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use crate::pdf::OpcoesPdf;
        use feather_ui::dialogo::{self as dlg, Dialogo};
        let muda = |f: &dyn Fn(&mut OpcoesPdf)| {
            let mut n = o.clone();
            f(&mut n);
            Msg::OpcoesPdf(n)
        };
        let campo = |dica: String, valor: &str, f: fn(&mut OpcoesPdf, String)| {
            let atual = o.clone();
            text_input(dica, valor.to_owned())
                .on_input(move |v| {
                    let mut n = atual.clone();
                    f(&mut n, v);
                    Msg::OpcoesPdf(n)
                })
                .on_submit(Msg::ConfirmarPdf)
                .padding([7, 10])
                .size(13)
                .placeholder_color(COR_TEXTO_MUTED())
                .selection_color(COR_HOVER_AZUL())
                .style(|_, status| {
                    let foco = status == Status::Focused;
                    Style::default()
                        .background(COR_PAINEL_FAIXA())
                        .text_color(COR_TEXTO())
                        .border(Border::rounded(6.).color(if foco { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() }, 1.))
                })
        };
        // Escolha de uma entre várias: etiqueta que acende, como as opções da busca.
        let etiqueta = |rotulo: String, ligada: bool, msg: Msg| {
            button(text(rotulo).size(12.5))
                .padding([5, 14])
                .on_press(msg)
                .style(move |_, status| Style::default()
                    .background(if ligada { COR_AZUL_FUNDO() } else if status == Status::Hovered { COR_HOVER() } else { Color::TRANSPARENT })
                    .text_color(if ligada { COR_AZUL_ACENTO() } else { COR_TEXTO() })
                    .border(Border::rounded(14.).color(if ligada { COR_AZUL_ACENTO() } else { COR_BORDA() }, 1.)))
        };
        let opcao = |icone_: &'static str, titulo: &str, dica: &str, liga: bool, f: fn(&mut OpcoesPdf, bool)| {
            let atual = o.clone();
            row![
                container(icone(icone_, 17.)).padding(8u16).style(|_| Style::default().background(COR_AZUL_FUNDO()).border(Border::rounded(9.))),
                feather_ui::configuracoes::linha(
                    tr(titulo),
                    tr(dica),
                    toggler("", liga).size(18.).on_toggle(move |v| {
                        let mut n = atual.clone();
                        f(&mut n, v);
                        Msg::OpcoesPdf(n)
                    }),
                ),
            ]
            .spacing(12)
            .align(Align::Center)
        };
        let total = self.quadros.len();
        let atual = (self.pagina_ativa + 1).to_string();
        let (resumo, valido) = match crate::pdf::paginas_do_intervalo(&o.paginas, total) {
            Ok(p) => (trf("{} de {} páginas", &[&p.len().to_string(), &total.to_string()]), true),
            Err(e) => (e, false),
        };
        let mut paginas = column![row![
            etiqueta(tr("Todas").into(), o.escolha_paginas == 0, muda(&|n| {
                n.escolha_paginas = 0;
                n.paginas.clear();
            })),
            etiqueta(trf("Página atual ({})", &[&atual]), o.escolha_paginas == 1, muda(&|n| {
                n.escolha_paginas = 1;
                n.paginas = atual.clone();
            })),
            etiqueta(tr("Intervalo").into(), o.escolha_paginas == 2, muda(&|n| {
                n.escolha_paginas = 2;
                if n.paginas.is_empty() {
                    n.paginas = format!("1-{total}");
                }
            })),
            horizontal_space(),
            text(resumo).size(12).color(if valido { COR_TEXTO_MUTED() } else { COR_ERRO() }),
        ]
        .spacing(8)
        .align(Align::Center)]
        .spacing(8);
        if o.escolha_paginas == 2 {
            paginas = paginas.push(campo(tr("ex.: 1-3, 5, 8").into(), &o.paginas, |n, v| n.paginas = v));
        }
        let predefinidas = [tr("RASCUNHO").to_string(), tr("CONFIDENCIAL").to_string()];
        let mut marcas = row![etiqueta(tr("Nenhuma").into(), o.marca.is_empty() && !o.marca_propria, muda(&|n| {
            n.marca.clear();
            n.marca_propria = false;
        }))]
        .spacing(8)
        .align(Align::Center);
        for m in &predefinidas {
            let m = m.clone();
            marcas = marcas.push(etiqueta(m.clone(), !o.marca_propria && o.marca == m, muda(&|n| {
                n.marca = m.clone();
                n.marca_propria = false;
            })));
        }
        marcas = marcas.push(etiqueta(tr("Outro texto").into(), o.marca_propria, muda(&|n| {
            n.marca_propria = true;
            n.marca.clear();
        })));
        let mut marca_dagua = column![marcas].spacing(8);
        if o.marca_propria {
            marca_dagua = marca_dagua.push(campo(tr("Texto na diagonal de cada página").into(), &o.marca, |n, v| n.marca = v));
        }
        let rotulado = |r: &str, c: pelicano::TextInput<'a, Msg>| column![text(tr(r).to_string()).size(11.5).color(COR_TEXTO_MUTED()), c].spacing(4).width(Length::FILL);
        let dados = column![
            rotulado("Título", campo(String::new(), &o.titulo, |n, v| n.titulo = v)),
            row![
                rotulado("Autor", campo(String::new(), &o.autor, |n, v| n.autor = v)),
                rotulado("Assunto", campo(String::new(), &o.assunto, |n, v| n.assunto = v)),
            ]
            .spacing(12),
            rotulado("Palavras-chave", campo(tr("separadas por vírgula").into(), &o.palavras_chave, |n, v| n.palavras_chave = v)),
        ]
        .spacing(10);
        let leva = column![
            opcao("link", "Links clicáveis", "Sumário, referências e endereços levam ao destino.", o.links, |n, v| n.links = v),
            opcao("sitemap", "Marcadores", "Os títulos na lateral do leitor de PDF.", o.marcadores, |n, v| n.marcadores = v),
            opcao("text-size", "Fontes da tela", "Embutidas no arquivo: o PDF fica igual ao que você vê.", o.fontes_da_tela, |n, v| n.fontes_da_tela = v),
            opcao("eye", "Abrir depois de exportar", "No leitor de PDF do sistema.", o.abrir_depois, |n, v| n.abrir_depois = v),
        ];
        let corpo = column![
            column![dlg::secao(&tr("Páginas")), paginas].spacing(8),
            column![dlg::secao(&tr("Marca d'água")), marca_dagua].spacing(8),
            column![dlg::secao(&tr("Dados do documento")), dados].spacing(8),
            column![dlg::secao(&tr("O PDF leva")), leva].spacing(0),
        ]
        .spacing(18);
        let nome = self.caminho.as_ref().and_then(|c| c.file_name()).map_or_else(|| tr("Documento sem nome").to_string(), |n| n.to_string_lossy().into_owned());
        Dialogo::new(tr("Exportar PDF"), corpo)
            .icone("file-export")
            .subtitulo(nome)
            .ao_fechar(Msg::Cancelar)
            .largura(520.0)
            .altura_maxima(800.0)
            .topo(70.0)
            .secundario(tr("cancelar"), Some(Msg::Cancelar))
            .primario(tr("Exportar"), valido.then_some(Msg::ConfirmarPdf))
            .sobre(base)
    }

    /// "Salvar as alterações?" por cima do documento (que continua à vista), no diálogo da suíte:
    /// o que se perde, e as três saídas; descartar fica à esquerda, longe do Salvar.
    fn view_salvar_pendente<'a>(&'a self, destino: &Destino, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use feather_ui::dialogo::{self as dlg, Dialogo};
        let nome = self.caminho.as_ref().and_then(|c| c.file_name()).map_or_else(|| tr("Documento sem nome").to_string(), |n| n.to_string_lossy().into_owned());
        let frase = match destino {
            Destino::Novo | Destino::Modelo(_) => tr("Antes de criar um documento novo, salve o que você mudou neste. Se descartar, as alterações se perdem."),
            Destino::Abrir | Destino::AbrirCaminho(_) => tr("Antes de abrir outro arquivo, salve o que você mudou neste. Se descartar, as alterações se perdem."),
            Destino::Fechar(_) => tr("Antes de fechar, salve o que você mudou neste documento. Se descartar, as alterações se perdem."),
        };
        Dialogo::new(tr("Salvar as alterações?"), text(frase).size(13).color(feather_ui::tema::COR_TEXTO()))
            .icone("device-floppy")
            .subtitulo(nome)
            .ao_fechar(Msg::Cancelar)
            .largura(460.0)
            .topo(170.0)
            .rodape_esquerda(dlg::botao_secundario(tr("descartar"), Some(Msg::Descartar)))
            .secundario(tr("cancelar"), Some(Msg::Cancelar))
            .primario(tr("salvar"), Some(Msg::SalvarEContinuar))
            .sobre(base)
    }
    /// O diálogo de editar a nota desenhada em (x, y) da página, se houver uma ali.
    fn nota_em(&self, pagina: usize, x: f32, y: f32) -> Option<crate::dialogos::Tipo> {
        let pos = self.sessao.nota_no_ponto(pagina, x, y)?;
        let fim = self.sessao.nota_info(&pos).ok()?.fim;
        Some(crate::dialogos::Tipo::EditarNota(pos, fim))
    }
    /// Segue um link: interno vai ao bloco; web e e-mail abrem no programa do sistema
    /// (só por Ctrl+clique, pedido explícito da pessoa).
    fn seguir_link(&mut self, link: &str) {
        if let Some(id) = featherdocs_core::referencias::destino_do_link(link) {
            self.sessao.ir_para_bloco(id);
            return;
        }
        let externo = ["http://", "https://", "mailto:"].iter().any(|p| link.starts_with(p));
        if !externo {
            self.status = trf("Link sem destino que eu saiba abrir: {}", &[&link]);
            return;
        }
        let r = abrir_no_sistema(link.as_ref());
        self.status = match r {
            Ok(_) => trf("Abrindo {}", &[&link]),
            Err(e) => trf("Não consegui abrir o link: {}", &[&e]),
        };
    }
    /// A largura da janela principal (a faixa decide o que encolhe por ela).
    pub(crate) fn largura_janela(&self) -> f32 {
        self.tamanho_principal.width
    }
    pub(crate) fn tamanho_janela(&self) -> Size {
        self.tamanho_principal
    }
    /// O último ponto do mouse sobre a faixa (o menu da caneta abre abaixo dele).
    pub(crate) fn ponteiro_da_faixa(&self) -> Point {
        self.ponteiro_faixa
    }
    /// Um menu que desce da faixa, logo abaixo dela, alinhado onde o botão foi clicado.
    fn view_menu_faixa<'a>(&'a self, base: Element<'a, Msg>, menu: faixa::MenuFaixa) -> Element<'a, Msg> {
        use faixa::MenuFaixa as M;
        // Menus clássicos (faixa recolhida): a lista desce da barra; a barra fica livre para o mouse
        // passar de um título a outro.
        if let M::Classico(m, sub) = menu {
            let topo = crate::menu_classico::ALTURA_BARRA - 2.;
            let painel = crate::menu_classico::painel(self, m, sub, self.tamanho_principal.height - 160.);
            return pelicano::stack![
                base,
                column![Space::new(0., topo), pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
                    .on_press(Msg::MenuFaixa(None)).on_right_press(Msg::MenuFaixa(None))],
                container(painel).padding(Padding { top: topo, left: crate::menu_classico::esquerda(m, self.tamanho_principal.width), ..Padding::ZERO }),
            ]
            .into();
        }
        let conteudo = match menu {
            M::Quebras => faixa::menu_quebras(),
            M::Estilos => faixa::menu_estilos(self),
            M::Grupo(i) => faixa::menu_grupo(self, i),
            M::Marcadores | M::Numeracao | M::Entrelinha | M::Bordas => faixa::menu_paragrafo(self, menu),
            _ => faixa::menu_pagina(self, menu),
        };
        // As abas (≈ 31 px) mais a faixa (106 px): o menu começa na borda de baixo da faixa.
        let topo = 138.;
        // A galeria aberta cobre a fechada (como no Word): a borda direita fica no botão que a abriu.
        let esquerda = match menu {
            M::Estilos => (self.ponto_menu_faixa.x + 24. - faixa::LARGURA_GALERIA_ABERTA).max(4.),
            // O grupo aberto não passa da borda direita da janela.
            M::Grupo(_) => (self.ponto_menu_faixa.x - 24.).min(self.tamanho_principal.width - 480.).max(4.),
            _ => (self.ponto_menu_faixa.x - 24.).max(4.),
        };
        pelicano::stack![
            base,
            pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
                .on_press(Msg::MenuFaixa(None))
                .on_right_press(Msg::MenuFaixa(None)),
            container(container(conteudo).padding(4u16).style(estilo_popup_menu))
                .padding(Padding { top: topo, left: esquerda, ..Padding::ZERO }),
        ]
        .into()
    }
    fn view_contexto<'a>(&'a self, base: Element<'a, Msg>, ponto: Point) -> Element<'a, Msg> {
        let item = |ic: &str, rotulo: &str, m: Msg| -> Element<'a, Msg> {
            button(row![icone(ic, 15.), text(tr(rotulo).to_string()).size(12)].spacing(8).align(Align::Center))
                .width(Length::FILL).padding([5, 10])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::DoMenu(Box::new(m)))
                .into()
        };
        let mut col = column![
            item("cut", tr("Recortar"), Msg::Comando(Comando::Recortar)),
            item("copy", tr("Copiar"), Msg::Comando(Comando::Copiar)),
            item("clipboard", tr("Colar"), Msg::Comando(Comando::Colar)),
            item("clipboard", tr("Colar sem formatação"), Msg::Comando(Comando::ColarSemFormatacao)),
        ].spacing(1);
        if self.sessao.link_no_cursor().is_some() {
            col = col
                .push(item("link-open", tr("Abrir link"), Msg::AbrirLink))
                .push(item("link", tr("Editar link"), Msg::FormAbrir(crate::dialogos::Tipo::Link)))
                .push(item("link-remove", tr("Remover link"), Msg::FormRemoverLink));
        } else {
            col = col.push(item("link", tr("Link…"), Msg::FormAbrir(crate::dialogos::Tipo::Link)));
        }
        if let Some(tipo) = self.nota_contexto {
            col = col.push(item("text-box", tr("Editar nota…"), Msg::FormAbrir(tipo)));
        }
        col = col
            .push(item("comentario", tr("Novo comentário…"), Msg::FormAbrir(crate::dialogos::Tipo::Comentario)))
            .push(item("clear", tr("Limpar formatação"), Msg::Comando(Comando::LimparFormatacao)));
        if self.visao.formato.tabela.is_some() {
            col = col
                .push(item("plus", tr("Inserir linha"), Msg::Comando(Comando::TabelaEstrutura("linha+"))))
                .push(item("plus", tr("Inserir coluna"), Msg::Comando(Comando::TabelaEstrutura("coluna+"))))
                .push(item("minus", tr("Excluir linha"), Msg::Comando(Comando::TabelaEstrutura("linha-"))))
                .push(item("minus", tr("Excluir coluna"), Msg::Comando(Comando::TabelaEstrutura("coluna-"))))
                .push(item("trash", tr("Excluir tabela"), Msg::Comando(Comando::TabelaExcluir)));
        }
        let menu = container(col).width(220).padding(4).style(estilo_popup_menu);
        pelicano::stack![
            base,
            pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
                .on_press(Msg::FecharContexto)
                .on_right_press(Msg::FecharContexto),
            // Opaco: o clique no vão entre os itens não chega à área de fechar, embaixo.
            container(pelicano::opaque(menu)).padding(Padding { top: ponto.y, left: ponto.x, ..Padding::ZERO }),
        ]
        .into()
    }
    fn view_revisao(&self) -> Element<'_, Msg> {
        use feather_ui::dialogo as dlg;
        let c = self.sessao.contagem();
        let mut col = column![
            row![text(tr("Revisão")).size(16), horizontal_space(),
                feather_ui::componentes::botao_icone(icone("x", 14.), "Fechar painel", Some(Msg::Comando(Comando::PainelRevisao)), false)],
            dlg::secao(tr(if c.selecao { "Estatísticas da seleção" } else { "Estatísticas" })),
            dlg::grade_rotulo_valor(vec![
                (tr("Páginas").into(), c.paginas.to_string()),
                (tr("Palavras").into(), c.palavras.to_string()),
                (tr("Caracteres").into(), c.caracteres.to_string()),
                (tr("Sem espaços").into(), c.caracteres_sem_espaco.to_string()),
                (tr("Parágrafos").into(), c.paragrafos.to_string()),
                (tr("Linhas").into(), c.linhas.to_string()),
            ], 2),
            dlg::secao(tr("Comentários")),
        ].spacing(8);
        let comentarios = self.sessao.comentarios();
        if comentarios.is_empty() {
            col = col.push(text(tr("Nenhum comentário.")).size(12).color(COR_TEXTO_MUTED()));
        }
        for k in comentarios {
            let mut cartao = column![
                text(format!("{} · p. {}", if k.autor.is_empty() { tr("Comentário") } else { &k.autor }, k.pagina)).size(11).color(COR_TEXTO_MUTED()),
                text(format!("“{}”", k.trecho)).size(11).color(COR_TEXTO_MUTED()),
                text(k.texto.clone()).size(13),
            ].spacing(3);
            for r in &k.respostas {
                cartao = cartao.push(container(text(r.texto.clone()).size(12)).padding(Padding { left: 14.0, ..Padding::ZERO }));
            }
            cartao = cartao.push(row![
                feather_ui::componentes::botao_icone(icone("search", 14.), "Ir ao trecho", Some(Msg::ComentarioIr(k.id)), false),
                feather_ui::componentes::botao_icone(icone("message-circle", 14.), "Responder", Some(Msg::FormAbrir(crate::dialogos::Tipo::Resposta(k.id))), false),
                feather_ui::componentes::botao_icone(icone("check", 14.), if k.resolvido { "Reabrir" } else { "Resolver" }, Some(Msg::ComentarioResolver(k.id, !k.resolvido)), k.resolvido),
                feather_ui::componentes::botao_icone(icone("trash", 14.), "Excluir comentário", Some(Msg::ComentarioExcluir(k.id)), false),
            ].spacing(2));
            col = col.push(container(cartao).padding(8).width(Length::FILL).style(dlg::estilo_bloco));
        }
        col = col.push(dlg::secao(tr("Alterações sugeridas")));
        let revisoes = self.sessao.revisoes();
        if revisoes.is_empty() {
            col = col.push(text(tr("Nenhuma alteração sugerida.")).size(12).color(COR_TEXTO_MUTED()));
        }
        for r in revisoes {
            let tipo = match r.tipo {
                featherdocs_core::modelo::TipoDeRevisao::Insercao => "Inserido",
                featherdocs_core::modelo::TipoDeRevisao::Remocao => "Removido",
            };
            col = col.push(
                button(column![
                    text(format!("{tipo}{}", if r.autor.is_empty() { String::new() } else { format!(" · {}", r.autor) })).size(11).color(COR_TEXTO_MUTED()),
                    text(r.texto).size(12),
                ].spacing(2))
                .width(Length::FILL).padding([6, 8])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::IrBloco(r.bloco)),
            );
        }
        container(scrollable(col.padding(14)))
            .width(300)
            .height(Length::FILL)
            .style(estilo_painel_miniaturas)
            .into()
    }
    /// Configurações → LaTeX (`f` = 0) ou Markdown (1): o painel de código de cada um.
    fn view_config_codigo(&self, f: usize) -> Element<'_, Msg> {
        let temas = painel_codigo::TEMAS.iter().enumerate().fold(column![].spacing(4), |itens, (i, nome)| {
            // A amostra: um quadradinho de cada cor do tema, ao lado do nome.
            let amostra = painel_codigo::cores_do_tema(i).iter().fold(row![].spacing(2), |r, c| {
                let c = *c;
                r.push(container(Space::new(10u16, 10u16)).style(move |_t| Style::default().background(c)))
            });
            itens.push(button(row![amostra, text(*nome).size(12.5)].spacing(8).align(Align::Center)).padding([6, 12]).width(220)
                .style(estilo_botao_pequeno(self.config.tema_codigo[f] == i)).on_press(Msg::TemaCodigo(f, i)))
        });
        column![
            feather_ui::configuracoes::linha(tr("Números de linha"), tr("À esquerda do painel de código."),
                toggler("", self.config.numeros_de_linha[f]).on_toggle(move |v| Msg::NumerosDeLinha(f, v))),
            feather_ui::configuracoes::linha(tr("Tema do código"), tr("As cores do realce de sintaxe."), temas),
        ].spacing(10).into()
    }

    fn view_configuracoes(&self) -> Element<'_, Msg> {
        let secoes = ["Aparência", "Exibição", "Documento", "LaTeX", "Markdown", "Salvamento", "Atalhos", "Sobre"];
        let menu = feather_ui::configuracoes::menu(
            tr("Configurações"),
            secoes.iter().enumerate().map(|(i, nome)| (
                tr(nome).to_owned(),
                self.config_secao == i as u8,
                Msg::ConfigSecao(i as u8),
            )),
        );
        let opcao = |rotulo: &str, ativa: bool, mensagem: Msg| {
            button(text(rotulo.to_owned()).size(12.5)).padding([6, 13])
                .style(estilo_botao_pequeno(ativa)).on_press(mensagem)
        };
        let cores = ACENTOS.iter().enumerate().fold(row![].spacing(8), |itens, (i, (nome, _, _))| {
            itens.push(opcao(nome, self.config.acento == i, Msg::Acento(i)))
        });
        let corpo: Element<'_, Msg> = match self.config_secao {
            0 => column![
                feather_ui::configuracoes::linha(tr("Tema"), tr("Automático usa a aparência do sistema na abertura."),
                    row![
                        opcao(tr("Automático"), self.config.tema_auto, Msg::TemaAuto),
                        opcao(tr("Claro"), !self.config.tema_auto && !self.config.escuro, Msg::Escuro(false)),
                        opcao(tr("Escuro"), !self.config.tema_auto && self.config.escuro, Msg::Escuro(true))
                    ].spacing(4)),
                feather_ui::configuracoes::linha(tr("Idioma"), tr("Idioma dos menus, da faixa e das janelas."),
                    crate::localidade::IDIOMAS.iter().fold(row![].spacing(4), |itens, (codigo, nome)| {
                        itens.push(opcao(nome, crate::localidade::atual() == *codigo, Msg::Idioma(codigo)))
                    })),
                feather_ui::configuracoes::linha(tr("Cor de destaque"), tr("Acento para botões, abas e seleção."), cores),
                feather_ui::configuracoes::linha(tr("Logotipo"), tr("Azul de fábrica ou cor escolhida"),
                    opcao(tr("Restaurar azul padrão"), false, Msg::LogoPadrao)),
                feather_ui::configuracoes::linha(tr("Barra de título integrada"), tr("Minimizar, maximizar e fechar na barra própria. Aplica-se na próxima abertura."),
                    toggler("", self.config.barra_propria).on_toggle(Msg::BarraPropria)),
            ].spacing(10).into(),
            1 => column![
                feather_ui::configuracoes::linha(tr("Faixa de opções"), tr("Ctrl+F1 mostra ou recolhe a faixa."),
                    toggler("", !self.config.faixa_recolhida).on_toggle(|_| Msg::AlternarFaixa)),
                feather_ui::configuracoes::linha(tr("Estrutura"), tr("Painel com os títulos do documento."),
                    toggler("", self.config.estrutura).on_toggle(|_| Msg::Comando(Comando::Estrutura))),
                feather_ui::configuracoes::linha(tr("Marcas de formatação"), tr("Mostrar os símbolos invisíveis de edição."),
                    toggler("", self.sessao.estado().marcas).on_toggle(|_| Msg::Comando(Comando::Marcas))),
            ].spacing(10).into(),
            2 => column![
                feather_ui::configuracoes::linha(tr("Documento"), tr("Papel e paginação calculados pelo motor Rust."),
                    text(trf("{} · {} páginas", &[&self.sessao.estado().papel, &self.sessao.total_de_paginas()])).size(12.5)),
                feather_ui::configuracoes::linha(tr("Estrutura"), tr("Exibir títulos para navegação."),
                    toggler("", self.config.estrutura).on_toggle(|_| Msg::Comando(Comando::Estrutura))),
            ].spacing(10).into(),
            3 | 4 => self.view_config_codigo(usize::from(self.config_secao - 3)),
            5 => column![
                feather_ui::configuracoes::linha(tr("Arquivo nativo"), tr("O FeatherDocs salva localmente em .fdoz."),
                    text(tr(if self.sessao.estado().sujo { "Alterações pendentes" } else { "Salvo" })).size(12.5)),
                feather_ui::configuracoes::linha(tr("Salvar"), tr("Também é possível exportar para Word, Markdown e LaTeX."),
                    opcao(tr("Salvar agora"), false, Msg::Comando(Comando::Salvar))),
            ].spacing(10).into(),
            6 => column![
                text(tr("Atalhos de teclado")).size(17),
                text(tr("Ctrl+N Novo · Ctrl+O Abrir · Ctrl+S Salvar · Ctrl+Shift+S Salvar como")),
                text(tr("Ctrl+F Buscar · Ctrl+Z Desfazer · F1 Ajuda · F12 Monitor · Ctrl+F1 Recolher faixa")),
                text(tr("A personalização das combinações ainda será integrada ao registro global da suíte."))
                    .size(12).color(COR_TEXTO_MUTED()),
            ].spacing(12).into(),
            _ => column![
                text(tr("FeatherDocs")).size(22),
                text(tr("Editor de documentos da suíte FeatherWorks, nativo em Rust/Iced.")),
                text(tr("Arquivos e operações são locais. Não há conta, telemetria ou envio automático de conteúdo.")),
                text(tr("Tema, botões, menus e configurações vêm da biblioteca feather-ui.")),
            ].spacing(12).into(),
        };
        feather_ui::configuracoes::tela(
            menu,
            secoes[self.config_secao.min(7) as usize],
            corpo,
            None,
        )
    }

    fn view_ajuda(&self) -> Element<'_, Msg> {
        feather_ui::cromo::painel_utilitario(
            tr("Ajuda · FeatherDocs"),
            ajuda::modelo().view(&self.ajuda_estado).map(Msg::AjudaEvento),
            Msg::FecharAuxiliar(JanelaAux::Ajuda),
        )
    }

    /// Monitor F12: a janela inteira (memória, CPU, console) é do `feather_ui::monitor`, a mesma do
    /// FeatherSheet; aqui só entram as métricas do documento.
    fn view_monitor(&self) -> Element<'_, Msg> {
        let e = self.sessao.estado();
        let nome = self.caminho.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str())
            .map_or_else(|| tr("Documento sem título").to_string(), str::to_string);
        let metricas = feather_ui::monitor::Metricas {
            cartoes: vec![
                (tr("Páginas").into(), e.paginas.to_string(), tr("No documento atual").into()),
                (tr("Palavras").into(), e.palavras.to_string(), trf("{} caracteres", &[&e.caracteres])),
                (tr("Blocos").into(), e.blocos.to_string(), tr("Parágrafos, títulos, tabelas e imagens").into()),
            ],
            linhas: vec![
                (tr("Documento").into(), nome),
                (tr("Estado").into(), tr(if e.sujo { "Alterações não salvas" } else { "Salvo" }).into()),
                (tr("Modo").into(), tr(&e.modo_edicao).into()),
                (tr("Revisões").into(), tr(if e.tem_revisoes { "sim" } else { "não" }).into()),
                (tr("Versão do documento").into(), e.versao.to_string()),
            ],
            ocupado: self.ocupado.then(|| tr("Operação de arquivo em andamento").into()),
        };
        self.monitor.view(metricas, Msg::MonitorEvento)
    }

    /// Localizar e substituir num painel arredondado no topo da área do documento (o padrão de
    /// busca da suíte): ícone, campo sem borda, contagem, anterior/próximo e fechar; a seta da
    /// esquerda abre a linha de Substituir, e as opções são etiquetas que acendem.
    fn view_busca(&self) -> Element<'_, Msg> {
        use feather_ui::dialogo as dlg;
        let campo = |dica: String, valor: &str| {
            text_input(dica, valor.to_owned()).padding([6u16, 4]).size(13)
                .placeholder_color(COR_TEXTO_MUTED())
                .selection_color(COR_HOVER_AZUL())
                .style(|_, _| Style::default().background(Color::TRANSPARENT).text_color(COR_TEXTO()))
        };
        let botao = |nome: &str, dica: &str, msg: Option<Msg>| {
            pelicano::tooltip(
                button(icone(nome, 15.)).padding(5u16).style(estilo_botao_pequeno(false)).on_press_maybe(msg),
                tr(dica).to_string(),
                pelicano::TooltipPosition::Bottom,
            )
            .style(estilo_popup_menu)
        };
        let etiqueta = |rotulo: &str, ligada: bool, i: u8| {
            button(text(tr(rotulo).to_string()).size(11.5))
                .padding([3, 10])
                .on_press(Msg::BuscaOpcao(i))
                .style(move |_, status| Style::default()
                    .background(if ligada { COR_AZUL_FUNDO() } else if status == Status::Hovered { COR_HOVER() } else { Color::TRANSPARENT })
                    .text_color(if ligada { COR_AZUL_ACENTO() } else { COR_TEXTO_MUTED() })
                    .border(Border::rounded(10.).color(if ligada { COR_AZUL_ACENTO() } else { COR_BORDA() }, 1.)))
        };
        let tem = !self.consulta.is_empty();
        let localizar = row![
            botao(if self.substituir_visivel { "chevron-down" } else { "chevron-right" }, "Substituir", Some(Msg::AlternarSubstituir)),
            icone("search", 16.),
            campo(tr("Localizar no documento").into(), &self.consulta)
                .id("busca")
                .on_input(Msg::Consulta)
                .on_submit(Msg::BuscaIr(1))
                .width(Length::FILL),
            text(self.busca_contagem.as_str()).size(12).color(COR_TEXTO_MUTED()),
            botao("chevron-up", "Anterior", tem.then_some(Msg::BuscaIr(-1))),
            botao("chevron-down", "Próximo (Enter)", tem.then_some(Msg::BuscaIr(1))),
            botao("x", "Fechar (Esc)", Some(Msg::Cancelar)),
        ]
        .spacing(6)
        .align(Align::Center);
        let mut painel = column![localizar].spacing(6);
        if self.substituir_visivel {
            painel = painel.push(
                row![
                    Space::new(25u16, 0u16),
                    icone("edit", 16.),
                    campo(tr("Substituir por").into(), &self.substituicao).on_input(Msg::Substituicao).on_submit(Msg::Substituir(false)).width(Length::FILL),
                    dlg::botao_secundario(tr("Substituir"), tem.then_some(Msg::Substituir(false))),
                    dlg::botao_secundario(tr("Substituir tudo"), tem.then_some(Msg::Substituir(true))),
                ]
                .spacing(6)
                .align(Align::Center),
            );
        }
        painel = painel.push(
            row![
                Space::new(25u16, 0u16),
                etiqueta("Diferenciar maiúsculas", self.busca_opcoes.maiusculas, 0),
                etiqueta("Palavra inteira", self.busca_opcoes.palavra, 1),
                etiqueta("Expressão regular", self.busca_opcoes.regex, 2),
            ]
            .spacing(6),
        );
        let cartao = container(painel)
            .padding([8, 10])
            .max_width(620.)
            .style(|_| Style::default()
                .background(COR_PAINEL())
                .border(Border::rounded(12.).color(COR_BORDA_SUAVE(), 1.))
                .shadow(Shadow { color: COR_SOMBRA(), offset: Point::new(0., 2.), blur: 8. }));
        container(cartao).width(Length::FILL).align_x(Align::End).padding([6u16, 14]).into()
    }
}
impl pelicano::App for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        let tarefa = App::update(self, msg);
        // O título de cada janela segue o estado (o `title` do daemon do Iced).
        let mut tarefas = vec![tarefa];
        let abertas: Vec<WindowId> = self.janelas.principal.into_iter()
            .chain([JanelaAux::Configuracoes, JanelaAux::Ajuda, JanelaAux::Monitor, JanelaAux::PrimeirosPassos, JanelaAux::Novidades].into_iter().filter_map(|a| self.janelas.id(a)))
            .collect();
        for id in abertas {
            let titulo = self.titulo(id);
            if self.titulos.get(&id) != Some(&titulo) {
                self.titulos.insert(id, titulo.clone());
                tarefas.push(Task::window_of(id, WindowCmd::Title(titulo)));
            }
        }
        Task::batch(tarefas)
    }

    fn view(&self) -> Element<'_, Msg> {
        App::view(self, WindowId::MAIN)
    }

    fn view_window(&self, id: WindowId) -> Element<'_, Msg> {
        App::view(self, id)
    }

    fn theme(&self) -> pelicano::Theme {
        tema()
    }

    fn subscription(&self) -> Subscription<Msg> {
        App::subscription(self)
    }

    // Chamado antes do `on_window_event`, e só quando nenhum widget usou o evento.
    fn on_unhandled_event(&self, _id: WindowId, _ev: &Event) -> Option<Msg> {
        self.evento_livre.set(true);
        None
    }

    fn on_window_event(&self, id: WindowId, ev: &Event) -> Option<Msg> {
        self.evento_janela(id, ev, self.evento_livre.take())
    }
}

fn nome_tecla(k: &keyboard::Key) -> String {
    use keyboard::{key::Named, Key};
    match k {
        Key::Character(c) => c.to_string(),
        Key::Named(n) => match n {
            Named::ArrowLeft => "ArrowLeft",
            Named::ArrowRight => "ArrowRight",
            Named::ArrowUp => "ArrowUp",
            Named::ArrowDown => "ArrowDown",
            Named::Enter => "Enter",
            Named::Backspace => "Backspace",
            Named::Delete => "Delete",
            Named::Home => "Home",
            Named::End => "End",
            Named::PageUp => "PageUp",
            Named::PageDown => "PageDown",
            Named::Tab => "Tab",
            Named::Escape => "Escape",
            _ => "",
        }
        .into(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ui013_ajuda_nativa_pesquisa_e_navega_sem_editar_documento() {
        let mut app=App::default();
        let antes=app.sessao.documento().clone();
        let _=app.update(Msg::AjudaEvento(feather_ui::ajuda::Evento::IrSecao(1)));
        assert_eq!(app.ajuda_estado.secao,1);
        let _=app.update(Msg::AjudaEvento(
            feather_ui::ajuda::Evento::Buscar("Ctrl+Shift+V".into())
        ));
        assert_eq!(app.ajuda_estado.busca,"Ctrl+Shift+V");
        assert!(ajuda::modelo().quantidade_encontrada(&app.ajuda_estado.busca)>0);
        let _=app.update(Msg::AjudaEvento(feather_ui::ajuda::Evento::Anterior));
        assert!(app.ajuda_estado.busca.is_empty());
        assert_eq!(*app.sessao.documento(),antes);
    }
    #[test]
    fn ui010_foco_por_clique_impede_digitacao_fora_do_documento() {
        let mut app = App::default();
        let documento = app.sessao.documento().clone();
        app.foco_documento = false;
        let _ = app.update(Msg::Tecla(
            keyboard::Key::Character("z".into()),
            keyboard::Modifiers::default(), Some("z".into()),
        ));
        assert_eq!(app.sessao.documento(), &documento);

        // Mouse ativa foco do editor, mas não insere nem modifica texto sozinho.
        let _ = app.update(Msg::Clicar(0, 20.0, 20.0, false, 1));
        assert!(app.foco_documento);
        assert_eq!(app.sessao.documento(), &documento);
    }

    #[test]
    fn ui010_atalho_de_janela_auxiliar_nao_modifica_o_documento() {
        use keyboard::{key::Named, Key, Modifiers};
        let mut app = App::default();
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::AtalhoJanela(principal, Key::Named(Named::F1), Modifiers::default()));
        let ajuda = app.janelas.id(JanelaAux::Ajuda).expect("Ajuda deve abrir");
        let _ = app.update(Msg::AtalhoJanela(ajuda, Key::Named(Named::F12), Modifiers::default()));
        assert!(app.janelas.aberta(JanelaAux::Ajuda), "F12 não fecha a Ajuda");
        assert!(!app.janelas.aberta(JanelaAux::Monitor), "F12 não vaza da janela auxiliar");
        let _ = app.update(Msg::AtalhoJanela(ajuda, Key::Named(Named::Escape), Modifiers::default()));
        assert!(!app.janelas.aberta(JanelaAux::Ajuda));
        assert_eq!(app.sessao.documento(), &original);
    }
    #[test]
    fn pincel_na_ribbon_ativa_e_esc_cancela_sem_alterar_documento() {
        let mut app = App::default();
        let antes = app.sessao.documento().clone();
        let _ = app.update(Msg::Comando(Comando::Pincel));
        assert!(app.sessao.pincel_cheio());
        assert_eq!(app.sessao.documento(), &antes);
        let _ = app.update(Msg::Cancelar);
        assert!(!app.sessao.pincel_cheio());
        assert_eq!(app.sessao.documento(), &antes);
    }

    #[test]
    fn pincel_sem_selecao_cancela_no_segundo_clique() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Pincel));
        assert!(app.sessao.pincel_cheio());
        let _ = app.update(Msg::Comando(Comando::Pincel));
        assert!(!app.sessao.pincel_cheio());
        assert!(app.status.contains("cancelado"));
    }

    #[test]
    fn atalhos_globais_preservam_conteudo_e_auxiliares_independentes() {
        use keyboard::{key::Named, Key, Modifiers};
        let mut app = App::default();
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::AtalhoJanela(principal, Key::Named(Named::F1), Modifiers::default()));
        assert!(app.janelas.aberta(JanelaAux::Ajuda));
        let id = app.janelas.id(JanelaAux::Ajuda).unwrap();
        let _ = app.update(Msg::AtalhoJanela(id, Key::Named(Named::F1), Modifiers::default()));
        assert!(!app.janelas.aberta(JanelaAux::Ajuda));
        let _ = app.update(Msg::AtalhoJanela(principal, Key::Named(Named::F12), Modifiers::default()));
        assert!(app.janelas.aberta(JanelaAux::Monitor));
        let id = app.janelas.id(JanelaAux::Monitor).unwrap();
        let _ = app.update(Msg::AtalhoJanela(id, Key::Named(Named::Escape), Modifiers::default()));
        assert!(!app.janelas.aberta(JanelaAux::Monitor));
        let antes = app.config.faixa_recolhida;
        let _ = app.update(Msg::AtalhoJanela(principal, Key::Named(Named::F1), Modifiers::CONTROL));
        assert_ne!(app.config.faixa_recolhida, antes);
        assert_eq!(app.sessao.documento(), &original);
    }
    #[test]
    fn editor_e_janelas_auxiliares_rendem_independentes() {
        let mut app = App::default();
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        for aba in Aba::TODAS {
            app.aba = aba;
            let _ = app.view(principal);
        }
        for tipo in [JanelaAux::Configuracoes, JanelaAux::Ajuda, JanelaAux::Monitor] {
            let id = window::novo_id();
            app.janelas.registrar(id, tipo);
            assert!(app.janelas.aberta(tipo));
            let _ = app.view(id);
            app.janelas.retirar(id);
        }
        app.busca = true;
        app.config.zoom = 2.;
        let _ = app.view(principal);
        app.pendente = Some(Destino::Novo);
        let _ = app.view(principal);
    }
    fn digitar(app: &mut App, s: &str) {
        let _ = app.update(Msg::Tecla(
            keyboard::Key::Character(s.into()),
            keyboard::Modifiers::default(),
            Some(s.into()),
        ));
    }
    #[test]
    fn cromo_global_conserva_documento_e_fecha_apenas_auxiliares() {
        let mut app = App::default();
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        digitar(&mut app, "Preservar documento");
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::Comando(Comando::Menu));
        assert!(app.menu_aberto);
        let _ = app.update(Msg::Comando(Comando::Configuracoes));
        assert!(app.janelas.aberta(JanelaAux::Configuracoes));
        assert!(!app.menu_aberto);
        let config_id = app.janelas.id(JanelaAux::Configuracoes).unwrap();
        let _ = app.view(config_id);
        let _ = app.update(Msg::ConfigSecao(4));
        let _ = app.view(config_id);
        let _ = app.update(Msg::AlternarFaixa);
        assert!(app.config.faixa_recolhida);
        let _ = app.update(Msg::Comando(Comando::Ajuda));
        assert!(app.janelas.aberta(JanelaAux::Ajuda));
        assert!(app.janelas.aberta(JanelaAux::Configuracoes));
        let ajuda_id = app.janelas.id(JanelaAux::Ajuda).unwrap();
        let _ = app.view(ajuda_id);
        let _ = app.update(Msg::AtalhoJanela(ajuda_id,
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::default()));
        assert!(!app.janelas.aberta(JanelaAux::Ajuda));
        let _ = app.update(Msg::Comando(Comando::Monitor));
        assert!(app.janelas.aberta(JanelaAux::Monitor));
        let monitor_id = app.janelas.id(JanelaAux::Monitor).unwrap();
        let _ = app.view(monitor_id);
        let _ = app.update(Msg::Fechar(monitor_id));
        assert!(!app.janelas.aberta(JanelaAux::Monitor));
        assert!(app.janelas.aberta(JanelaAux::Configuracoes));
        assert_eq!(app.janelas.principal, Some(principal));
        assert_eq!(app.sessao.documento(), &original);
    }
    #[test]
    fn edicao_nativa_preserva_unicode_formato_e_historico() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Formato(
            featherdocs_core::modelo::NEGRITO,
        )));
        digitar(&mut app, "Olá Rust");
        assert_eq!(
            featherdocs_io::exportar::texto_simples(app.sessao.documento()),
            "Olá Rust"
        );
        assert!(app.visao.formato.negrito);
        assert!(app.quadros[0].operacoes.iter().any(|o| matches!(o,Operacao::Texto {estilo,..} if estilo & featherdocs_core::modelo::NEGRITO != 0)));
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(
            featherdocs_io::exportar::texto_simples(app.sessao.documento()),
            ""
        );
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(
            featherdocs_io::exportar::texto_simples(app.sessao.documento()),
            "Olá Rust"
        );
    }
    #[test]
    fn novo_e_fechamento_pedem_salvamento_e_cancelar_preserva_edicoes() {
        let mut app = App::default();
        digitar(&mut app, "Não perder");
        let antes = app.sessao.documento().clone();
        let _ = app.update(Msg::Comando(Comando::Novo));
        assert!(matches!(app.pendente, Some(Destino::Novo)));
        let _ = app.update(Msg::Fonte("serifa".into()));
        let _ = app.update(Msg::Cancelar);
        assert_eq!(app.sessao.documento(), &antes);
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        let _ = app.update(Msg::Fechar(principal));
        assert!(matches!(app.pendente, Some(Destino::Fechar(id)) if id == principal));
        let _ = app.update(Msg::Cancelar);
        assert_eq!(app.sessao.documento(), &antes);
    }
    #[test]
    fn qa009_cancelar_abertura_e_erro_preservam_documento_sujo() {
        let mut app = App::default();
        digitar(&mut app, "Texto inalterado no cancelamento");
        let antes = app.sessao.documento().clone();
        app.ocupado = true;
        let _ = app.update(Msg::Aberto(Ok(None)));
        assert!(!app.ocupado);
        assert_eq!(app.sessao.documento(), &antes);
        assert!(app.status.contains("cancelada"));
        app.ocupado = true;
        let _ = app.update(Msg::Aberto(Err("Documento corrompido".into())));
        assert!(!app.ocupado);
        assert_eq!(app.sessao.documento(), &antes);
        assert!(app.status.contains("corrompido"));
    }

    #[test]
    fn abrir_falha_e_exportar_nao_apagam_documento_sujo() {
        let mut app = App::default();
        digitar(&mut app, "Texto original");
        let antes = app.sessao.documento().clone();
        let _ = app.update(Msg::Aberto(Err("Arquivo inválido".into())));
        assert_eq!(app.sessao.documento(), &antes);
        let v = app.sessao.versao();
        let _ = app.update(Msg::Gravado(Ok(Some((
            PathBuf::from("copia.docx"),
            v,
            true,
        )))));
        assert!(app.sessao.estado().sujo);
        assert!(app.caminho.is_none());
        let _ = app.update(Msg::Gravado(Ok(Some((
            PathBuf::from("documento.fdoz"),
            v,
            false,
        )))));
        assert!(!app.sessao.estado().sujo);
    }
    #[test]
    fn busca_substituicao_e_tabela_usam_motor() {
        let mut app = App::default();
        digitar(&mut app, "ação ação");
        let _ = app.update(Msg::Consulta("ação".into()));
        let _ = app.update(Msg::Substituicao("edição".into()));
        let _ = app.update(Msg::Substituir(true));
        assert_eq!(
            featherdocs_io::exportar::texto_simples(app.sessao.documento()),
            "edição edição"
        );
        let _ = app.update(Msg::Cancelar);
        let _ = app.update(Msg::Comando(Comando::Tabela));
        assert!(app.sessao.documento().blocos.iter().any(|b| matches!(
            b.conteudo,
            featherdocs_core::modelo::Conteudo::Tabela { .. }
        )));
        let bytes = featherdocs_io::fdoz::gravar(app.sessao.documento()).unwrap();
        assert_eq!(
            featherdocs_io::fdoz::ler(&bytes).unwrap(),
            *app.sessao.documento()
        );
    }
    #[test]
    fn tabela_no_ribbon_usa_tables_e_preserva_undo_e_fdoz() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Tabela));
        let original = app.sessao.documento().clone();
        let paginas = app.sessao.total_de_paginas();
        let _ = app.update(Msg::Comando(Comando::TabelaEstrutura("linha+")));
        let apos_linha = app.sessao.documento().clone();
        assert_ne!(apos_linha, original);
        let _ = app.update(Msg::Comando(Comando::TabelaEstrutura("coluna+")));
        let atualizado = app.sessao.documento().clone();
        assert_ne!(atualizado, apos_linha);
        assert!(app.sessao.total_de_paginas() >= paginas);
        assert_eq!(
            featherdocs_io::fdoz::ler(&featherdocs_io::fdoz::gravar(&atualizado).unwrap()).unwrap(),
            atualizado
        );
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &apos_linha);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &original);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &apos_linha);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &atualizado);
    }

    #[test]
    fn mesclar_e_desmesclar_na_gui_preservam_undo_e_fdoz() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Tabela));
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::Comando(Comando::TabelaMesclar));
        let mesclado = app.sessao.documento().clone();
        assert_ne!(mesclado, original);
        assert_eq!(mesclado.blocos.iter()
            .find(|b| matches!(b.conteudo, featherdocs_core::modelo::Conteudo::Tabela { .. }))
            .unwrap().mesclagens.len(), 1);
        let bytes = featherdocs_io::fdoz::gravar(&mesclado).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&bytes).unwrap(), mesclado);
        let _ = app.update(Msg::Comando(Comando::TabelaDesmesclar));
        assert_eq!(app.sessao.documento(), &original);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &mesclado);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &original);
    }

    #[test]
    fn app006_cabecalho_exclusao_undo_e_round_trip_fdoz() {
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Tabela));
        let original = app.sessao.documento().clone();
        assert!(app.sessao.tabela_tem_cabecalho());
        assert!(app.visao.formato.tabela.is_some());

        let _ = app.update(Msg::Comando(Comando::TabelaCabecalho));
        assert!(!app.sessao.tabela_tem_cabecalho());
        let sem_cabecalho = app.sessao.documento().clone();
        assert_ne!(original, sem_cabecalho);
        let fdoz = featherdocs_io::fdoz::gravar(&sem_cabecalho).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&fdoz).unwrap(), sem_cabecalho);

        let _ = app.update(Msg::Comando(Comando::TabelaExcluir));
        assert!(app.sessao.documento().blocos.iter().all(|b|
            !matches!(b.conteudo, featherdocs_core::modelo::Conteudo::Tabela { .. })
        ));
        // O contador de ids não volta no desfazer (evita reaproveitar id); o conteúdo volta exato.
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento().blocos, sem_cabecalho.blocos);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento().blocos, original.blocos);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento().blocos, sem_cabecalho.blocos);
    }

    #[test]
    fn app006_edicao_estrutural_preserva_trechos_ricos_e_paginacao() {
        use featherdocs_core::modelo::{Conteudo, Documento, Trecho, NEGRITO};
        let mut negrito = Trecho::simples("Cabeçalho");
        negrito.estilo = NEGRITO;
        negrito.link = Some("https://example.org".into());
        let mut documento = Documento::vazio();
        let id = documento.acrescentar(Conteudo::Tabela {
            cabecalho: true,
            linhas: vec![
                vec![vec![negrito.clone()], vec![Trecho::simples("Dado")]],
                vec![vec![Trecho::simples("Linha 2")], vec![]],
            ],
        });
        let mut app = App::default();
        app.sessao = Sessao::com_documento(documento);
        app.atualizar();
        let antes = app.sessao.documento().clone();
        let paginas_antes = app.sessao.total_de_paginas();
        assert_eq!(app.sessao.cursor().foco.bloco, id);
        assert_eq!(app.sessao.cursor().foco.celula, Some((0, 0)));

        let _ = app.update(Msg::Comando(Comando::TabelaEstrutura("coluna+")));
        let _ = app.update(Msg::Comando(Comando::TabelaEstrutura("linha+")));
        let depois = app.sessao.documento().clone();
        let bloco = depois.blocos.iter().find(|b| b.id == id).unwrap();
        let Conteudo::Tabela { linhas, cabecalho } = &bloco.conteudo else {
            panic!("O bloco deixou de ser tabela.");
        };
        assert!(*cabecalho);
        assert_eq!((linhas.len(), linhas[0].len()), (3, 3));
        assert_eq!(linhas[0][0], vec![negrito]);
        assert_eq!(linhas[0][2][0].texto, "Dado");
        assert_eq!(linhas[2][0][0].texto, "Linha 2");
        assert!(app.sessao.total_de_paginas() >= paginas_antes);
        assert!(app.visao.formato.tabela.is_some());

        let fdoz = featherdocs_io::fdoz::gravar(&depois).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&fdoz).unwrap(), depois);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &antes);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &depois);
    }

    #[test]
    fn app006_mescla_rejeita_celula_ocupada_sem_apagar_dados() {
        use featherdocs_core::modelo::{Conteudo, Documento, Trecho};
        let mut documento = Documento::vazio();
        documento.acrescentar(Conteudo::Tabela {
            cabecalho: true,
            linhas: vec![vec![
                vec![Trecho::simples("A")],
                vec![Trecho::simples("B")],
            ]],
        });
        let mut app = App::default();
        app.sessao = Sessao::com_documento(documento);
        app.atualizar();
        let antes = app.sessao.documento().clone();
        let _ = app.update(Msg::Comando(Comando::TabelaMesclar));
        assert_eq!(app.sessao.documento(), &antes);
        assert!(!app.status.is_empty(), "a GUI deve explicar por que a mesclagem falhou");
        let _ = app.update(Msg::Comando(Comando::TabelaCabecalho));
        assert_ne!(app.sessao.documento(), &antes);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &antes);
    }

    #[test]
    fn img014_preserva_id_legenda_fdoz_e_undo_da_imagem() {
        use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;
        let mut png = Vec::new();
        let pixels = RgbaImage::from_fn(2, 2, |x, y| Rgba([
            if x == 0 { 255 } else { 0 }, if y == 0 { 0 } else { 255 }, 0, 128
        ]));
        DynamicImage::ImageRgba8(pixels)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png).unwrap();
        let (origem, w, h) = crate::imagem_compartilhada::preparar_insercao(&png).unwrap();
        let mut app = App::default();
        app.sessao.inserir_imagem(&origem, w, h).unwrap();
        let id = app.sessao.documento().blocos.iter()
            .find(|b| matches!(b.conteudo, featherdocs_core::modelo::Conteudo::Imagem { .. }))
            .unwrap().id;
        app.sessao.ir_para_bloco(id);
        app.atualizar();
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::AbrirEfeitosImagem);
        assert!(app.efeito_imagem.is_some());
        let _ = app.update(Msg::AplicarEfeitoImagem("cinza"));
        let alterado = app.sessao.documento().clone();
        assert_ne!(alterado, original);
        assert!(alterado.blocos.iter().any(|b| b.id == id));
        let bytes = featherdocs_io::fdoz::gravar(&alterado).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&bytes).unwrap(), alterado);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento(), &original);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &alterado);
        let _ = app.update(Msg::AplicarEfeitoImagem("original"));
        assert_eq!(app.sessao.documento(), &original);
        let _ = app.update(Msg::FecharEfeitosImagem);
        assert!(app.efeito_imagem.is_none());
    }

    #[test]
    fn img014_sem_imagem_nao_modifica_documento() {
        let mut app = App::default();
        let original = app.sessao.documento().clone();
        let _ = app.update(Msg::AbrirEfeitosImagem);
        assert!(app.efeito_imagem.is_none());
        let _ = app.update(Msg::AplicarEfeitoImagem("cinza"));
        assert_eq!(app.sessao.documento(), &original);
    }

    #[test]
    fn app008_seletor_nativo_edita_texto_realce_e_preserva_fdoz_undo() {
        let mut app = App::default();
        digitar(&mut app, "Texto colorido");
        app.sessao.selecionar_tudo();
        app.atualizar();
        let antes = app.sessao.documento().clone();

        let _ = app.update(Msg::CorEscolhida(AlvoCor::Texto, "#112233".into()));
        assert_eq!(app.visao.formato.cor, "#112233");
        let _ = app.update(Msg::CorEscolhida(AlvoCor::Realce, "#ffee00".into()));
        assert_eq!(app.visao.formato.realce, "#ffee00");
        let colorido = app.sessao.documento().clone();
        assert_ne!(colorido, antes);
        let bytes = featherdocs_io::fdoz::gravar(&colorido).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&bytes).unwrap(), colorido);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_ne!(app.sessao.documento(), &colorido);
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert_eq!(app.sessao.documento(), &colorido);
    }

    #[test]
    fn app008_cor_invalida_nao_modifica_documento() {
        let mut app = App::default();
        digitar(&mut app, "Teste");
        app.sessao.selecionar_tudo();
        app.atualizar();
        let antes = app.sessao.documento().clone();
        let _ = app.update(Msg::CorEscolhida(AlvoCor::Texto, "#zz".into()));
        assert_eq!(app.sessao.documento(), &antes);
        assert!(!app.status.is_empty());
    }

    #[test]
    fn cht016_ribbon_insere_grafico_real_e_reabre_fdoz() {
        use featherdocs_core::modelo::{Conteudo, Documento, Trecho};
        use featherdocs_io::grafico_docs::{eh_grafico, Tipo};
        let celula = |s: &str| vec![Trecho::simples(s)];
        let mut doc = Documento::vazio();
        doc.acrescentar(Conteudo::Tabela {
            cabecalho: true,
            linhas: vec![
                vec![celula("Mês"), celula("Consumo")],
                vec![celula("Jan"), celula("12,5")],
                vec![celula("Fev"), celula("13")],
            ],
        });
        let mut app = App::default();
        app.sessao = Sessao::com_documento(doc);
        app.atualizar();
        let _ = app.update(Msg::Comando(Comando::Grafico(Tipo::Colunas)));
        let grafico = app.sessao.documento().blocos.iter()
            .find(|b| matches!(b.conteudo, Conteudo::Imagem { .. }))
            .expect("a Ribbon não inseriu um bloco de gráfico");
        let id = grafico.id;
        let Conteudo::Imagem { origem, .. } = &grafico.conteudo else { unreachable!() };
        assert!(eh_grafico(origem), "a Ribbon não utilizou o adaptador Charts");
        let salvo = featherdocs_io::fdoz::gravar(app.sessao.documento()).unwrap();
        assert_eq!(featherdocs_io::fdoz::ler(&salvo).unwrap(), *app.sessao.documento());

        app.sessao.ir_para_bloco(id);
        let versao_antes = app.sessao.versao();
        let _ = app.update(Msg::Comando(Comando::AtualizarGrafico));
        assert_eq!(app.sessao.versao(), versao_antes, "gráfico igual não deve sujar o documento");
        assert_eq!(app.sessao.documento().blocos.iter().filter(|b| b.id == id).count(), 1);
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        // Atualização idempotente não adiciona um passo; Undo desfaz a inserção.
        assert!(!app.sessao.documento().blocos.iter().any(|b| b.id == id));
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert!(app.sessao.documento().blocos.iter().any(|b| b.id == id));
    }

    /// Digita `texto` no fim do editor de código e faz a pausa passar.
    fn digitar_no_codigo(app: &mut App, texto: &str) {
        // O editor do pelicano edita o `Content` sozinho e avisa com `on_change`: aqui o texto muda
        // como se a pessoa colasse no fim, e chegam o clique e o aviso.
        let _ = app.update(Msg::CodigoFoco);
        let painel = app.codigo.as_mut().unwrap();
        painel.conteudo = pelicano::Content::with_text(&(painel.conteudo.text() + texto));
        let _ = app.update(Msg::CodigoMudou);
    }
    /// Desenha a janela uma vez: o editor de código só tem cursor depois de montado.
    fn montar_codigo(app: &App) {
        let _ = pelicano::capture(&app.view(WindowId::MAIN), Size::new(1200., 800.), 1., &app.tema(WindowId::MAIN), &mut pelicano::Renderer::new(), None);
    }
    fn vencer_pausa(app: &mut App) {
        if let Some(p) = app.codigo.as_mut() {
            p.ultima_edicao = p.ultima_edicao.map(|t| t - painel_codigo::PAUSA);
        }
        let _ = app.update(Msg::CodigoPulso);
    }

    #[test]
    fn codigo_abrir_gera_latex_e_alterna_markdown() {
        let mut app = App::default();
        digitar(&mut app, "Olá mundo");
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Latex)));
        let texto = app.codigo.as_ref().unwrap().conteudo.text();
        assert!(texto.contains("\\begin{document}"), "{texto}");
        assert!(texto.contains("Olá mundo"));
        assert!(!app.codigo.as_ref().unwrap().blocos.is_empty());
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        assert_eq!(app.fonte_do_codigo(), Some(Fonte::Markdown));
        assert_eq!(app.codigo.as_ref().unwrap().conteudo.text().trim(), "Olá mundo");
        let principal = window::novo_id();
        app.janelas.principal = Some(principal);
        let _ = app.view(principal);
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        assert!(app.codigo.is_none(), "o mesmo botão desliga o painel");
    }

    #[test]
    fn codigo_editado_vai_ao_documento_depois_da_pausa_e_desfaz_num_passo() {
        let mut app = App::default();
        digitar(&mut app, "Primeiro");
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        let antes = app.sessao.documento().clone();
        digitar_no_codigo(&mut app, "\n\n## Segundo título\n");
        assert!(!app.foco_documento, "editar o código tira o foco da página");
        assert!(app.codigo.as_ref().unwrap().pendente());
        // O pulso antes da pausa não aplica nada.
        let _ = app.update(Msg::CodigoPulso);
        assert_eq!(app.sessao.documento(), &antes);
        vencer_pausa(&mut app);
        assert!(!app.codigo.as_ref().unwrap().pendente());
        assert!(app.sessao.documento().blocos.iter().any(|b| matches!(
            &b.conteudo, featherdocs_core::modelo::Conteudo::Titulo { nivel: 2, .. })));
        assert!(app.status.contains("aplicado"), "{}", app.status);
        // O texto da pessoa continua como ela escreveu.
        assert!(app.codigo.as_ref().unwrap().conteudo.text().contains("## Segundo título"));
        // Ctrl+Z (pelo editor) desfaz a aplicação inteira e o código volta.
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento().blocos, antes.blocos);
        assert!(!app.codigo.as_ref().unwrap().conteudo.text().contains("Segundo"));
        let _ = app.update(Msg::Comando(Comando::Refazer));
        assert!(app.codigo.as_ref().unwrap().conteudo.text().contains("## Segundo título"));
    }

    #[test]
    fn codigo_pendente_e_aplicado_antes_de_editar_a_pagina() {
        let mut app = App::default();
        digitar(&mut app, "Base");
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        digitar_no_codigo(&mut app, "\n\nDo código\n");
        // Clicar na página antes da pausa aplica o código primeiro.
        let _ = app.update(Msg::Clicar(0, 20.0, 20.0, false, 1));
        let texto = featherdocs_io::exportar::texto_simples(app.sessao.documento());
        assert!(texto.contains("Do código"), "{texto}");
        assert!(app.foco_documento);
    }

    #[test]
    fn codigo_regenera_quando_a_pagina_muda_e_segue_o_bloco_clicado() {
        let mut app = App::default();
        digitar(&mut app, "Um");
        let _ = app.update(Msg::Tecla(keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::default(), None));
        digitar(&mut app, "Dois");
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Latex)));
        app.foco_documento = true;
        digitar(&mut app, " três");
        let painel = app.codigo.as_ref().unwrap();
        assert!(painel.conteudo.text().contains("Dois três"));
        assert_eq!(painel.versao, app.sessao.versao());
        // O cursor do código está na linha do bloco do cursor da página.
        let id = app.sessao.cursor().foco.bloco;
        let de = painel.blocos.iter().find(|b| b.id == id).unwrap().de;
        montar_codigo(&app);
        let painel = app.codigo.as_ref().unwrap();
        assert_eq!(painel.linha_do_cursor(), de);
        assert_eq!(painel.conteudo.line(de + 1).unwrap().trim(), "Dois três");
        // Ir ao primeiro bloco pela estrutura leva o código junto.
        let primeiro = app.sessao.documento().blocos[0].id;
        let _ = app.update(Msg::IrBloco(primeiro));
        let painel = app.codigo.as_ref().unwrap();
        let de = painel.blocos.iter().find(|b| b.id == primeiro).unwrap().de;
        assert_eq!(painel.linha_do_cursor(), de);
    }

    #[test]
    fn codigo_clicado_leva_a_pagina_ao_bloco() {
        let mut app = App::default();
        digitar(&mut app, "Um");
        let _ = app.update(Msg::Tecla(keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::default(), None));
        digitar(&mut app, "Dois");
        let primeiro = app.sessao.documento().blocos[0].id;
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        assert_ne!(app.sessao.cursor().foco.bloco, primeiro);
        // O clique no editor: o aviso do clique, o cursor no começo e, ao soltar, a conferência do bloco.
        montar_codigo(&app);
        let _ = app.update(Msg::CodigoFoco);
        app.codigo.as_ref().unwrap().conteudo.go_to_line(1);
        let _ = app.update(Msg::CodigoCursor);
        assert_eq!(app.sessao.cursor().foco.bloco, primeiro);
        assert!(!app.foco_documento);
    }

    #[test]
    fn colar_tex_no_painel_de_documento_vazio_da_o_mesmo_que_abrir_o_arquivo() {
        // O cabeçalho do currículo de teste do usuário: faixa azul com nome e subtítulo.
        let tex = "\\documentclass{article}\n\\usepackage{xcolor}\n\\definecolor{darkblue}{HTML}{1A3A5C}\n\\definecolor{lightblue}{HTML}{D6E8F7}\n\\begin{document}\n\\noindent\n\\colorbox{darkblue}{%\n  \\begin{minipage}{\\dimexpr\\textwidth-2\\fboxsep\\relax}\n    \\vspace{8pt}\n    \\centering\n    \\textbf{\\LARGE\\color{white}Marcelo A. Gonçalves}\\\\[3pt]\n    {\\color{lightblue}Engenheiro Eletricista}\n    \\vspace{8pt}\n  \\end{minipage}%\n}\\par\n\\begin{center}\n  {\\small Vitória -- ES}\n\\end{center}\nTexto do perfil.\n\\end{document}\n";
        let lido = featherdocs_io::latex::ler(tex);
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Latex)));
        // Selecionar tudo e colar: o editor troca o texto inteiro e avisa.
        let _ = app.update(Msg::CodigoFoco);
        app.codigo.as_mut().unwrap().conteudo = pelicano::Content::with_text(tex);
        let _ = app.update(Msg::CodigoMudou);
        vencer_pausa(&mut app);
        let d = app.sessao.documento();
        assert_eq!(d.blocos.len(), lido.blocos.len());
        for (a, b) in d.blocos.iter().zip(&lido.blocos) {
            assert_eq!((&a.conteudo, a.atrib), (&b.conteudo, b.atrib));
        }
        assert_ne!(d.blocos[0].atrib.fundo, 0, "o nome fica na faixa (não some branco no branco)");
        assert_eq!(d.blocos[0].atrib.fundo, d.blocos[1].atrib.fundo, "uma faixa só para nome e subtítulo");
    }

    #[test]
    fn abrir_tex_mostra_o_texto_original_no_painel() {
        let tex = "\\documentclass{article}\n% meu comentário\n\\begin{document}\nTexto do arquivo.\n\\end{document}\n";
        let p = std::env::temp_dir().join(format!("featherdocs-codigo-{}.tex", std::process::id()));
        std::fs::write(&p, tex).unwrap();
        let doc = crate::arquivos::ler(&p).unwrap();
        let mut app = App::default();
        let _ = app.update(Msg::Comando(Comando::Codigo(Fonte::Markdown)));
        let _ = app.update(Msg::Aberto(Ok(Some((p.clone(), doc)))));
        std::fs::remove_file(&p).unwrap();
        let painel = app.codigo.as_ref().unwrap();
        assert_eq!(painel.fonte, Fonte::Latex, "o painel passa à língua do arquivo");
        assert_eq!(painel.conteudo.text(), tex.trim_end_matches('\n').to_string() + "\n");
        assert!(app.caminho.is_none(), "salvar continua pedindo um .fdoz");
    }

    #[test]
    fn formularios_aplicam_no_motor_e_erro_mantem_aberto() {
        use crate::dialogos::Tipo;
        let mut app = App::default();
        let _ = app.update(Msg::FormAbrir(Tipo::Margens));
        assert!(!app.foco_documento);
        let _ = app.update(Msg::FormCampo(0, "3,0".into()));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_none() && app.foco_documento);
        assert!((app.sessao.documento().pagina.margem_esq - 3.0 * 72.0 / 2.54).abs() < 0.01);

        let _ = app.update(Msg::FormAbrir(Tipo::Margens));
        let _ = app.update(Msg::FormCampo(1, "abc".into()));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_some(), "campo inválido não fecha o diálogo");
        let _ = app.update(Msg::Cancelar);
        assert!(app.formulario.is_none());

        let _ = app.update(Msg::FormAbrir(Tipo::Cabecalho));
        let _ = app.update(Msg::FormCampo(2, "{titulo}".into()));
        let _ = app.update(Msg::FormOk);
        assert_eq!(app.sessao.documento().extras.cabecalho.direita, "{titulo}");

        digitar(&mut app, "x");
        let _ = app.update(Msg::FormAbrir(Tipo::Link));
        let _ = app.update(Msg::FormCampo(0, "https://exemplo.org".into()));
        let _ = app.update(Msg::FormCampo(1, "site".into()));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_none());
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        let _ = app.update(Msg::Comando(Comando::Desfazer));
        assert_eq!(app.sessao.documento().extras.cabecalho.direita, "{titulo}");
    }

    #[test]
    fn menu_do_botao_direito_executa_e_fecha() {
        let mut app = App::default();
        digitar(&mut app, "abc");
        let _ = app.update(Msg::Rolou(Vector::new(0.0, 100.0)));
        // O ponto já vem na janela: a rolagem não entra na conta (no Iced vinha no conteúdo rolado).
        let _ = app.update(Msg::BotaoDireito(0, 20.0, 20.0, Point::new(50.0, 300.0)));
        assert_eq!(app.menu_contexto, Some(Point::new(50.0, 300.0)));
        let _ = app.update(Msg::DoMenu(Box::new(Msg::FormAbrir(crate::dialogos::Tipo::Comentario))));
        assert!(app.menu_contexto.is_none() && app.formulario.is_some());
        let _ = app.update(Msg::Cancelar);
        let _ = app.update(Msg::BotaoDireito(0, 20.0, 20.0, Point::new(50.0, 300.0)));
        let _ = app.update(Msg::Cancelar);
        assert!(app.menu_contexto.is_none());
    }

    #[test]
    fn nova_fonte_e_citacao_pelos_dialogos() {
        use crate::dialogos::Tipo;
        let mut app = App::default();
        let _ = app.update(Msg::FormAbrir(Tipo::NovaFonte));
        for (i, v) in [(0, "silva2023"), (1, "artigo"), (2, "Silva, Ana; Souza, Bia"), (3, "Um título"), (4, "2023")] {
            let _ = app.update(Msg::FormCampo(i, v.into()));
        }
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_none(), "{}", app.status);
        let f = &app.sessao.documento().bibliografia.fontes[0];
        assert_eq!((f.chave.as_str(), f.autores.len(), f.autores[1].nome.as_str()), ("silva2023", 2, "Bia"));

        let _ = app.update(Msg::FormAbrir(Tipo::Citacao));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_some(), "sem fonte marcada não insere");
        let _ = app.update(Msg::FormCampo(0, "silva2023".into()));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_none(), "{}", app.status);
        assert!(format!("{:?}", app.sessao.documento().blocos).contains("(Silva; Souza, 2023)"));
    }

    #[test]
    fn nota_inserida_e_editada_pelo_dialogo() {
        use crate::dialogos::Tipo;
        let mut app = App::default();
        let _ = app.update(Msg::FormAbrir(Tipo::NotaRodape));
        let _ = app.update(Msg::FormCampo(0, "primeira".into()));
        let _ = app.update(Msg::FormOk);
        let bloco = app.sessao.documento().blocos[0].id;
        let pos = featherdocs_core::cursor::Pos { bloco, celula: None, desl: 0 };
        assert_eq!(app.sessao.nota_info(&pos).unwrap().fonte, "primeira");
        let _ = app.update(Msg::FormAbrir(Tipo::EditarNota(pos, false)));
        assert_eq!(app.formulario.as_ref().unwrap().campos[0], "primeira");
        let _ = app.update(Msg::FormCampo(0, "segunda".into()));
        let _ = app.update(Msg::FormOk);
        assert_eq!(app.sessao.nota_info(&pos).unwrap().fonte, "segunda");
    }

    #[test]
    fn novo_estilo_guarda_a_formatacao_e_exclui() {
        use crate::dialogos::Tipo;
        let mut app = App::default();
        digitar(&mut app, "abc");
        let _ = app.update(Msg::Comando(Comando::Alinhar("centro")));
        let _ = app.update(Msg::FormAbrir(Tipo::NovoEstilo));
        let _ = app.update(Msg::FormCampo(0, "Destaque".into()));
        let _ = app.update(Msg::FormOk);
        assert!(app.formulario.is_none(), "{}", app.status);
        let e = &app.sessao.documento().estilos.lista[0];
        assert_eq!((e.nome.as_str(), e.alinhamento), ("Destaque", featherdocs_core::modelo::Alinhamento::Centro));
        assert_eq!(app.sessao.estilo_com_nome_no_cursor(), e.id);
        let _ = app.update(Msg::Comando(Comando::EstiloExcluir));
        assert!(app.sessao.documento().estilos.lista.is_empty(), "{}", app.status);
    }

    #[test]
    fn recentes_sem_repetir_e_no_maximo_dez() {
        let mut c = crate::configuracao::Configuracao::default();
        for i in 0..12 {
            c.lembrar(std::path::Path::new(&format!("/tmp/{i}.fdoz")));
        }
        c.lembrar(std::path::Path::new("/tmp/5.fdoz"));
        assert_eq!(c.recentes.len(), 10);
        assert_eq!(c.recentes[0], PathBuf::from("/tmp/5.fdoz"));
        assert_eq!(c.recentes.iter().filter(|p| p.ends_with("5.fdoz")).count(), 1);
    }

    #[test]
    fn recursos_embutidos_e_localidade_cobrem_a_faixa() {
        let dados: serde_json::Value =
            serde_json::from_str(include_str!("../dados/pt-BR.json")).unwrap();
        for chave in [
            "inicio",
            "salvar",
            "negrito",
            "tabela",
            "configuracoes",
            "salvar_pendente",
        ] {
            assert!(dados.get(chave).is_some());
        }
        assert!(crate::pagina::carregar_imagem("https://exemplo.com/imagem.png").is_none());
        assert!(crate::pagina::carregar_imagem("/tmp/imagem.png").is_none());
    }
}

/// Os diálogos de arquivo começam na pasta FeatherDocs dos Documentos,
/// se ela existir.
/// Abre um endereço ou arquivo no programa que o sistema escolhe (navegador, leitor de PDF).
/// Abre `alvo` no programa do sistema; uma thread espera o processo terminar, senão ele fica "defunto" até o app fechar.
/// O painel Estrutura: largura padrão, mínima, e abaixo de quanto o arrasto o fecha (pixels da janela).
pub const LARGURA_ESTRUTURA: f32 = 220.0;
const LARGURA_ESTRUTURA_MIN: f32 = 140.0;
const LARGURA_ESTRUTURA_SOME: f32 = 60.0;

pub(crate) fn abrir_no_sistema(alvo: &std::ffi::OsStr) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(alvo).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(alvo).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(alvo).spawn();
    let mut filho = r?;
    std::thread::spawn(move || filho.wait());
    Ok(())
}

fn dialogo_de_arquivo() -> rfd::AsyncFileDialog {
    let casa = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let pasta = casa.and_then(|c| ["Documentos", "Documents"].iter().map(|d| c.join(d).join("FeatherDocs")).find(|p| p.is_dir()));
    match pasta {
        Some(p) => rfd::AsyncFileDialog::new().set_directory(p),
        None => rfd::AsyncFileDialog::new(),
    }
}

/// O que o painel mostra de um arquivo do projeto.
pub enum Visto {
    Texto(String),
    /// A imagem na resolução original e o tamanho dela em pixels.
    Imagem(pelicano::Image, u32, u32),
    Aviso(String),
}

/// A data de hoje por extenso no idioma da interface ("18 de outubro de 2026").
/// shortcut: conta o dia em UTC (sem fuso); perto da meia-noite pode mostrar o dia vizinho.
fn hoje_por_extenso() -> String {
    let dias = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Dias desde 1970-01-01 → ano, mês e dia do calendário civil.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    let i = (mes - 1) as usize;
    match crate::localidade::atual() {
        "en-US" => format!("{} {dia}, {ano}", ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][i]),
        "es-ES" => format!("{dia} de {} de {ano}", ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"][i]),
        _ => format!("{dia} de {} de {ano}", ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"][i]),
    }
}

/// A tela sozinha numa pilha, como fica sob um menu aberto: o estado do pelicano (a rolagem do
/// documento) é guardado pelo caminho na árvore, e abrir ou fechar um menu não pode mudar esse caminho.
fn so(base: Element<'_, Msg>) -> Element<'_, Msg> {
    pelicano::stack![base].width(Length::FILL).height(Length::FILL).into()
}

/// O tema escuro pedido: o escolhido, ou no automático o do sistema.
fn tema_escuro(automatico: bool, escuro: bool) -> bool {
    // shortcut: o `dark_light` do pelicano dá claro no GNOME com `color-scheme` = prefer-dark; aqui se
    // pergunta ao gsettings também. Tirar quando o pelicano detectar direito.
    let gnome = || {
        std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "color-scheme"]).output()
            .is_ok_and(|s| String::from_utf8_lossy(&s.stdout).contains("dark"))
    };
    escuro_resolvido(automatico, escuro) || automatico && cfg!(target_os = "linux") && gnome()
}
