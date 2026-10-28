# Migração featherdocs-gui (Iced 0.13) → featherdocs-pelicano

Inventário do `featherdocs-gui` feito antes da porta (commit `d625069`). Cada item diz como ficou
no crate novo:

- **portado** — mesmo comportamento;
- **shortcut** — portado com um limite marcado no código como `// shortcut:` (motivo ao lado);
- **faltando** — não portado (motivo ao lado).

Status: **porta concluída**. Total: 41 linhas de inventário (janelas, diálogos, faixa por aba, atalhos, captura; cada linha
agrupa vários controles) + 7 seções de Configurações + 86 `Msg` + 75 `Comando`. **Faltando: 0.** Linhas com shortcut: 3; shortcuts no código: 6 novos + 3 que já vinham do gui (seção 8).
Testes: os 67 do gui, menos 1 que era do `Highlighter` do Iced (`realce_reinicia_da_linha_alterada`),
mais 1 do `compat.rs` = 67 passando.

## 1. Janelas

| Item | Status |
|---|---|
| Janela principal (1200×800, moldura do sistema ou barra própria, fechar pergunta antes) | portado |
| Configurações (900×640) | portado |
| Ajuda F1 (980×700) | portado |
| Monitor F12 (980×640, pulso de 1 s) | portado |
| Título da janela com `*` quando sujo | portado |
| Arrastar/maximizar pela barra própria (duplo clique), minimizar, fechar | portado |
| Soltar arquivo na janela abre o documento | portado |

## 2. Diálogos e painéis por cima do documento

| Item | Status |
|---|---|
| Formulário (`dialogos::Tipo`): Link, NotaRodape, NotaFim, Comentario, Cabecalho, Rodape, MarcaDagua, Marcador, IndiceEntrada, Referencia, Margens, Papel, BordaPagina, Propriedades, Tabela, NovaFonte, Citacao, EditarNota, NovoEstilo, Resposta, AplicarEstilo | portado |
| Foco no primeiro campo ao abrir o formulário | portado |
| Exportar PDF (páginas, marca d'água, dados, o que o PDF leva) | portado |
| Salvar as alterações? (Novo / Abrir / Fechar) | portado |
| Aviso de perdas ao exportar / ver em Markdown | portado |
| Seletor de cor (texto, marca-texto, sombreamento, página) | portado |
| Efeitos de imagem | portado |
| Localizar e substituir (opções maiúsculas / palavra / regex, contagem, anterior/próximo) | **shortcut** (letra das dicas fixa) |
| Menu do botão direito (recortar, copiar, colar, link, nota, comentário, limpar, tabela) | portado |
| Menus que descem da faixa: Quebras, Estilos (galeria inteira), Marcadores, Numeração, Entrelinha, Bordas | portado |
| Menu Arquivo (lateral) com Recentes | portado |
| Painel Estrutura (títulos / Arquivos do projeto / Avisos do LaTeX) | portado |
| Painel de código LaTeX/Markdown (modo dividido, pausa de 300 ms, segue o bloco) | **shortcut** (ver 8: realce, números de linha, rolagem com margem, edição com app ocupado) |
| Visualizador de arquivo do projeto (imagem com zoom, texto com números de linha) | portado |
| Painel Revisão (estatísticas, comentários, alterações sugeridas) | portado |
| Rodapé (página, palavras, status, zoom) | portado |

## 3. Faixa (por aba)

| Aba | Itens | Status |
|---|---|---|
| Início | Colar, Recortar, Copiar, Pincel de formatação, Colar sem formatação; fonte, tamanho, Aa (5 modos); Negrito, Itálico, Sublinhado, Tachado, Sobrescrito, Subscrito, Código, Aumentar/Diminuir fonte, Limpar formatação; Cor do texto, Marca-texto; Parágrafo (lista com marcadores + tipo, numerada + tipo, recuos, alinhamento 4, espaçamento, sombreamento, bordas); galeria de estilos (4 visíveis, setas, todos os estilos, novo/atualizar/excluir estilo) | portado |
| Inserir | Tabela + Tamanho (8), Imagem, Efeitos imagem; Gráficos (Colunas, Barras, Linhas, Área, Pizza, Dispersão, Degraus, Atualizar selecionado); Quebra, Equação, Linha horizontal; Link, Comentário, Marcador, Referência cruzada; Cabeçalho, Rodapé, Número de página (8); Editar tabela (Linha ±, Coluna ±, Mesclar, Desmesclar); Cabeçalho da tabela, Excluir tabela | portado |
| Desenhar | Selecionar, Caneta, Marca-texto, Borracha, Laço, Desenhar com o toque (desabilitados, "Em breve") | portado |
| Design | Cor da página, Borda da página, Marca d'água | portado |
| Layout | Margens (6), Papel personalizado, Texto na página (3); A4, Carta; Retrato, Paisagem; Quebras; recuos, espaçamento; Números de linha (3), Primeira página diferente | portado |
| Referências | Sumário + menu (4), Atualizar sumário; nota de rodapé, nota de fim; Nova fonte, Importar .bib, Inserir citação, Bibliografia (2), Estilo (5); Inserir legenda (3), Índice de ilustrações (2), Referência cruzada; Marcar entrada, Inserir índice | portado |
| Correspondências | Envelopes, Etiquetas, Iniciar mala direta, Destinatários ("Em breve") | portado |
| Revisão | Localizar; Editar, Sugerir, Leitura; Aceitar, Rejeitar, Aceitar todas, Rejeitar todas, Comparar com arquivo; Novo comentário, Painel de revisão; Proteger | portado |
| Exibir | Estrutura, Marcas, Código LaTeX, Código Markdown; Reduzir, Ampliar; Configurações | portado |
| Barra do editor | Menu, Novo, Abrir, Salvar, Exportar, Desfazer, Refazer, Pesquisar, Ajuda, Monitor, Configurações | portado |

## 4. Atalhos de teclado

| Atalho | Status |
|---|---|
| Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+P, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+V, Ctrl+F, Ctrl+K, Ctrl+, , Esc | portado |
| F1 (Ajuda), F12 (Monitor), Ctrl+F1 (recolher faixa) — globais da suíte | portado |
| Na Ajuda: Ctrl+F foca a busca; Esc/F1 fecha. No Monitor/Configurações: Esc fecha | portado |
| Teclas do documento (setas, Home/End, PgUp/PgDn, Enter, Backspace, Delete, Tab, Ctrl+B/I/U…, texto) pelo motor (`Sessao::tecla`) | portado |
| Ctrl+roda: zoom; Ctrl+clique: seguir link / sumário; duplo clique numa nota: editar; clique duplo/triplo: palavra/linha | portado |
| No painel de código: Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y vão ao desfazer do documento | portado |

## 5. Mensagens (`Msg`) e comandos (`Comando`)

`Msg` (86): Atalho Comando Aba Fonte Tamanho AbrirCor EventoCor AjudaEvento Tecla Clicar Soltar
MonitorEvento MonitorPulso MenuFaixa PonteiroFaixa IrBloco Colado ColadoPuro FormAbrir AbrirRecente
BibLido BotaoDireito CtrlClique AbrirLink DoMenu FecharContexto Rolou Comparado ComentarioIr
ComentarioResolver ComentarioExcluir FormCampo FormOk FormFechar FormRemoverLink FormEscolher Aberto
Gravado PerdasAoExportar ExportarMesmoAssim FecharPerdas OpcoesPdf ConfirmarPdf Imagem
AbrirEfeitosImagem AplicarEfeitoImagem FecharEfeitosImagem Fechar FecharAuxiliar JanelaAberta
JanelaFechada ArrastarJanela Minimizar AlternarMaximizar AtalhoJanela BarraPropria NumerosDeLinha
TemaCodigo CodigoRolou Lateral IrLinhaCodigo AbrirArquivo FecharArquivo ZoomArquivo Descartar
SalvarEContinuar Cancelar Escuro TemaAuto Acento Idioma LogoPadrao ConfigSecao AlternarFaixa Consulta
Substituicao BuscaOpcao BuscaIr AlternarSubstituir Substituir CodigoEditar CodigoPulso CodigoFechar
GaleriaEstilos CapturarTela TelaCapturada

`Comando` (75): Novo Abrir Salvar SalvarComo Exportar ExportarPdf Desfazer Refazer Copiar Recortar
Colar Pincel ColarSemFormatacao Formato Posicao TamanhoPasso LimparFormatacao Maiusculas Recuar Lista
ListaReiniciar Entrelinha Espaco Borda NumeroPagina NumeroRemover Legenda IndiceInserir ListaDe
SumarioRemover SumarioNiveis ReferenciasAtualizar Margens NumerarLinhas PrimeiraDiferente Vertical
Proteger Formulario ImportarBib Bibliografia EstiloCitacao Estilo EstiloNome EstiloAtualizar
EstiloExcluir Alinhar Tabela TabelaTam Grafico AtualizarGrafico TabelaEstrutura TabelaMesclar
TabelaDesmesclar TabelaCabecalho TabelaExcluir Imagem Quebra QuebraDeLinha EstiloRapido Sumario Papel
Orientacao Modo Revisoes PainelRevisao Comparar Estrutura Marcas Codigo Zoom Configuracoes Buscar
Menu Ajuda Monitor

## 6. Configurações (seções)

Aparência (tema, idioma, cor de destaque, logotipo, barra integrada), Exibição (faixa, estrutura,
marcas), Documento, LaTeX e Markdown (números de linha, tema do código), Salvamento, Atalhos, Sobre —
status: portado.

## 7. Modo captura (`FEATHERDOCS_CAPTURAR*`)

| Variável | Status |
|---|---|
| `FEATHERDOCS_CAPTURAR=arquivo.png` (espera abrir, maximiza, fotografa e fecha) | **shortcut** (a foto sai sem a rolagem da página e do código) |
| `_TEMA`, `_CODIGO`, `_ZOOM`, `_BLOCO`, `_ABA`, `_PENDENTE`, `_BUSCA`, `_ESTILOS`, `_EXPORTAR_PDF`, `_LINHA`, `_ARQUIVO`, `_PDF` | portado |

### Mudanças de mensagem (mesmo comportamento, outro caminho)

- `JanelaAberta`, `JanelaFechada`: saíram. O pelicano não avisa janela aberta, e fechar pelo sistema
  chega como `Event::CloseRequested` → `Msg::Fechar(id)` (o mesmo de antes).
- `TelaCapturada`: saiu. A foto é feita direto (`pelicano::capture`) no segundo pulso de `CapturarTela`.
- `CodigoEditar(Action)` virou três: `CodigoMudou` (o `on_change` do editor), `CodigoFoco` (clique no
  editor, pelo `compat::ao_clicar`) e `CodigoCursor` (no fim de cada tecla/clique, confere se o
  cursor do código foi a outro bloco e leva a página junto).
- `CodigoRolou`: saiu. O editor do pelicano rola sozinho até o cursor (o do Iced ficava numa barra de
  rolagem de fora, e o app seguia o cursor com ela).
- Nova: `JanelaRedimensionada(Size)`, para a foto do modo captura sair no tamanho da janela.
- `BotaoDireito`: o ponto vem na janela (o Iced dava no conteúdo rolado e o app descontava a rolagem).
- `Msg::Tecla`/`AtalhoJanela` usam as teclas do winit pelo `compat::keyboard` (mesmos nomes).

Todos os 75 `Comando` ficaram iguais.

## 8. Shortcuts deixados (`// shortcut:` no código)

1. `painel_codigo.rs`: o editor só pinta pelo syntect (`tex`/`md` + tema). O realce do motor
   (`featherdocs_io::realce`) e os 4 temas não entram; `destaque()` escolhe o tema do syntect mais perto
   (o pelicano só tem um tema claro). Falta no pelicano: realce próprio (trechos por linha) no `text_editor`.
2. `painel_codigo.rs`: ir à linha só rola até ela aparecer (sem as 12 linhas de contexto). Falta:
   rolar com margem / "centralizar a linha".
3. `app.rs` (`CodigoMudou`): com o app ocupado a edição do código não é barrada (o Iced descartava a
   ação). Falta: `text_editor` só leitura.
4. `app.rs` (`view_codigo`): os números de linha são do pelicano, sem cor por linha; a linha com aviso do
   LaTeX vai sublinhada em onda (com a dica) em vez do número vermelho. Falta: cor por número de linha.
5. `faixa.rs` (e as outras dicas): o `tooltip` do pelicano não escolhe o tamanho da letra (era 11). Falta: `size` no `tooltip`.
6. `app.rs` (`capturar_tela`): a foto monta a árvore com estado novo, então sai sem a rolagem. Falta:
   fotografar o quadro da janela.
7–9. Os três que já estavam no gui (estilos de caractere, prévia SVG/GIF 800×600, data em UTC).

## 9. Diferenças visuais que sobraram (sem código)

- A letra da interface é a sans do sistema: o pelicano não tem "fonte padrão" para trocar (o Iced usava
  a Liberation Sans embutida). A página continua na Liberation (pedida pelo nome).
- A faixa não mostra o trilho cinza da barra de rolagem horizontal quando cabe inteira.
- Rótulos do gráfico SVG um pouco mais finos (rasterização do pelicano).
- Os interruptores (`toggler`) saem na cor de destaque do tema da suíte (o Iced os pintava de azul).
- As cores do código são as do tema syntect (shortcut 1), não as paletas do motor.
- Na foto do modo captura a página e o código ficam no topo (shortcut 6); na janela de verdade rolam até o cursor.

Fotos comparadas (vitrine.tex, 1920×1048): Início, painel de código LaTeX, Exportar PDF, busca +
galeria de estilos aberta — layout, faixa, diálogos e páginas iguais ao Iced fora o listado acima.
