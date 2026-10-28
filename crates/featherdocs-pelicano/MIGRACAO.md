# Migração featherdocs-gui (Iced 0.13) → featherdocs-pelicano

Inventário do `featherdocs-gui` feito antes da porta (commit `d625069`). Cada item diz como ficou
no crate novo:

- **portado** — mesmo comportamento;
- **shortcut** — portado com um limite marcado no código como `// shortcut:` (motivo ao lado);
- **faltando** — não portado (motivo ao lado).

Status: **EM ANDAMENTO** (preenchido ao fim da porta).

## 1. Janelas

| Item | Status |
|---|---|
| Janela principal (1200×800, moldura do sistema ou barra própria, fechar pergunta antes) | pendente |
| Configurações (900×640) | pendente |
| Ajuda F1 (980×700) | pendente |
| Monitor F12 (980×640, pulso de 1 s) | pendente |
| Título da janela com `*` quando sujo | pendente |
| Arrastar/maximizar pela barra própria (duplo clique), minimizar, fechar | pendente |
| Soltar arquivo na janela abre o documento | pendente |

## 2. Diálogos e painéis por cima do documento

| Item | Status |
|---|---|
| Formulário (`dialogos::Tipo`): Link, NotaRodape, NotaFim, Comentario, Cabecalho, Rodape, MarcaDagua, Marcador, IndiceEntrada, Referencia, Margens, Papel, BordaPagina, Propriedades, Tabela, NovaFonte, Citacao, EditarNota, NovoEstilo, Resposta, AplicarEstilo | pendente |
| Foco no primeiro campo ao abrir o formulário | pendente |
| Exportar PDF (páginas, marca d'água, dados, o que o PDF leva) | pendente |
| Salvar as alterações? (Novo / Abrir / Fechar) | pendente |
| Aviso de perdas ao exportar / ver em Markdown | pendente |
| Seletor de cor (texto, marca-texto, sombreamento, página) | pendente |
| Efeitos de imagem | pendente |
| Localizar e substituir (opções maiúsculas / palavra / regex, contagem, anterior/próximo) | pendente |
| Menu do botão direito (recortar, copiar, colar, link, nota, comentário, limpar, tabela) | pendente |
| Menus que descem da faixa: Quebras, Estilos (galeria inteira), Marcadores, Numeração, Entrelinha, Bordas | pendente |
| Menu Arquivo (lateral) com Recentes | pendente |
| Painel Estrutura (títulos / Arquivos do projeto / Avisos do LaTeX) | pendente |
| Painel de código LaTeX/Markdown (modo dividido, pausa de 300 ms, segue o bloco) | pendente |
| Visualizador de arquivo do projeto (imagem com zoom, texto com números de linha) | pendente |
| Painel Revisão (estatísticas, comentários, alterações sugeridas) | pendente |
| Rodapé (página, palavras, status, zoom) | pendente |

## 3. Faixa (por aba)

| Aba | Itens | Status |
|---|---|---|
| Início | Colar, Recortar, Copiar, Pincel de formatação, Colar sem formatação; fonte, tamanho, Aa (5 modos); Negrito, Itálico, Sublinhado, Tachado, Sobrescrito, Subscrito, Código, Aumentar/Diminuir fonte, Limpar formatação; Cor do texto, Marca-texto; Parágrafo (lista com marcadores + tipo, numerada + tipo, recuos, alinhamento 4, espaçamento, sombreamento, bordas); galeria de estilos (4 visíveis, setas, todos os estilos, novo/atualizar/excluir estilo) | pendente |
| Inserir | Tabela + Tamanho (8), Imagem, Efeitos imagem; Gráficos (Colunas, Barras, Linhas, Área, Pizza, Dispersão, Degraus, Atualizar selecionado); Quebra, Equação, Linha horizontal; Link, Comentário, Marcador, Referência cruzada; Cabeçalho, Rodapé, Número de página (8); Editar tabela (Linha ±, Coluna ±, Mesclar, Desmesclar); Cabeçalho da tabela, Excluir tabela | pendente |
| Desenhar | Selecionar, Caneta, Marca-texto, Borracha, Laço, Desenhar com o toque (desabilitados, "Em breve") | pendente |
| Design | Cor da página, Borda da página, Marca d'água | pendente |
| Layout | Margens (6), Papel personalizado, Texto na página (3); A4, Carta; Retrato, Paisagem; Quebras; recuos, espaçamento; Números de linha (3), Primeira página diferente | pendente |
| Referências | Sumário + menu (4), Atualizar sumário; nota de rodapé, nota de fim; Nova fonte, Importar .bib, Inserir citação, Bibliografia (2), Estilo (5); Inserir legenda (3), Índice de ilustrações (2), Referência cruzada; Marcar entrada, Inserir índice | pendente |
| Correspondências | Envelopes, Etiquetas, Iniciar mala direta, Destinatários ("Em breve") | pendente |
| Revisão | Localizar; Editar, Sugerir, Leitura; Aceitar, Rejeitar, Aceitar todas, Rejeitar todas, Comparar com arquivo; Novo comentário, Painel de revisão; Proteger | pendente |
| Exibir | Estrutura, Marcas, Código LaTeX, Código Markdown; Reduzir, Ampliar; Configurações | pendente |
| Barra do editor | Menu, Novo, Abrir, Salvar, Exportar, Desfazer, Refazer, Pesquisar, Ajuda, Monitor, Configurações | pendente |

## 4. Atalhos de teclado

| Atalho | Status |
|---|---|
| Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+P, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+V, Ctrl+F, Ctrl+K, Ctrl+, , Esc | pendente |
| F1 (Ajuda), F12 (Monitor), Ctrl+F1 (recolher faixa) — globais da suíte | pendente |
| Na Ajuda: Ctrl+F foca a busca; Esc/F1 fecha. No Monitor/Configurações: Esc fecha | pendente |
| Teclas do documento (setas, Home/End, PgUp/PgDn, Enter, Backspace, Delete, Tab, Ctrl+B/I/U…, texto) pelo motor (`Sessao::tecla`) | pendente |
| Ctrl+roda: zoom; Ctrl+clique: seguir link / sumário; duplo clique numa nota: editar; clique duplo/triplo: palavra/linha | pendente |
| No painel de código: Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y vão ao desfazer do documento | pendente |

## 5. Mensagens (`Msg`) e comandos (`Comando`)

`Msg` (88): Atalho Comando Aba Fonte Tamanho AbrirCor EventoCor AjudaEvento Tecla Clicar Soltar
MonitorEvento MonitorPulso MenuFaixa PonteiroFaixa IrBloco Colado ColadoPuro FormAbrir AbrirRecente
BibLido BotaoDireito CtrlClique AbrirLink DoMenu FecharContexto Rolou Comparado ComentarioIr
ComentarioResolver ComentarioExcluir FormCampo FormOk FormFechar FormRemoverLink FormEscolher Aberto
Gravado PerdasAoExportar ExportarMesmoAssim FecharPerdas OpcoesPdf ConfirmarPdf Imagem
AbrirEfeitosImagem AplicarEfeitoImagem FecharEfeitosImagem Fechar FecharAuxiliar JanelaAberta
JanelaFechada ArrastarJanela Minimizar AlternarMaximizar AtalhoJanela BarraPropria NumerosDeLinha
TemaCodigo CodigoRolou Lateral IrLinhaCodigo AbrirArquivo FecharArquivo ZoomArquivo Descartar
SalvarEContinuar Cancelar Escuro TemaAuto Acento Idioma LogoPadrao ConfigSecao AlternarFaixa Consulta
Substituicao BuscaOpcao BuscaIr AlternarSubstituir Substituir CodigoEditar CodigoPulso CodigoFechar
GaleriaEstilos CapturarTela TelaCapturada — status: pendente.

`Comando` (73): Novo Abrir Salvar SalvarComo Exportar ExportarPdf Desfazer Refazer Copiar Recortar
Colar Pincel ColarSemFormatacao Formato Posicao TamanhoPasso LimparFormatacao Maiusculas Recuar Lista
ListaReiniciar Entrelinha Espaco Borda NumeroPagina NumeroRemover Legenda IndiceInserir ListaDe
SumarioRemover SumarioNiveis ReferenciasAtualizar Margens NumerarLinhas PrimeiraDiferente Vertical
Proteger Formulario ImportarBib Bibliografia EstiloCitacao Estilo EstiloNome EstiloAtualizar
EstiloExcluir Alinhar Tabela TabelaTam Grafico AtualizarGrafico TabelaEstrutura TabelaMesclar
TabelaDesmesclar TabelaCabecalho TabelaExcluir Imagem Quebra QuebraDeLinha EstiloRapido Sumario Papel
Orientacao Modo Revisoes PainelRevisao Comparar Estrutura Marcas Codigo Zoom Configuracoes Buscar
Menu Ajuda Monitor — status: pendente.

## 6. Configurações (seções)

Aparência (tema, idioma, cor de destaque, logotipo, barra integrada), Exibição (faixa, estrutura,
marcas), Documento, LaTeX e Markdown (números de linha, tema do código), Salvamento, Atalhos, Sobre —
status: pendente.

## 7. Modo captura (`FEATHERDOCS_CAPTURAR*`)

| Variável | Status |
|---|---|
| `FEATHERDOCS_CAPTURAR=arquivo.png` (espera abrir, maximiza, fotografa e fecha) | pendente |
| `_TEMA`, `_CODIGO`, `_ZOOM`, `_BLOCO`, `_ABA`, `_PENDENTE`, `_BUSCA`, `_ESTILOS`, `_EXPORTAR_PDF`, `_LINHA`, `_ARQUIVO`, `_PDF` | pendente |
