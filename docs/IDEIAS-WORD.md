# Reclamações do Word e o que o FeatherDocs faz

Pesquisa de 30/10/2026 (fóruns da Microsoft, Tech Community, Q&A e blogs; o Reddit quase não
apareceu nas buscas). A frequência é estimada, não medida.

Legenda: ✅ o FeatherDocs já faz · 🔧 em andamento · ⬜ falta.

## Já resolvido pelo jeito do FeatherDocs

- ✅ Ctrl+A e F9 para atualizar campos: sumário, legendas, listas de figuras e numeração de títulos são recalculados a cada mudança.
- ✅ Linha de tabela partida entre páginas: a paginação nunca parte uma linha de tabela.
- ✅ Colar de fora com a formatação da origem: o texto colado entra puro, no estilo do destino.
- ✅ Tirar a formatação manual e voltar ao estilo: Limpar formatação.
- ✅ "Editar dados" do gráfico depende do Excel e abre mal: a planilha está no próprio FeatherDocs, com intervalo A1:D5 digitado ou arrastado e prévia ao lado.

## Feito nesta rodada (30/10/2026)

- ✅ Recuperação contínua e local: cópia a cada 20 s do documento não salvo, oferecida ao abrir depois de uma queda.
- ✅ Editar comentário; "Resolvido por fulano em data".
- ✅ Galeria de estilos de tabela (8 prontos, um Ctrl+Z) e "Limpar formatação da tabela" (tabelas coladas).
- ✅ Foto maior que 2400 px é reduzida ao inserir, e a barra avisa.
- ✅ "Fixar na página": a imagem sai do texto e fica no lugar.
- ✅ Ao abrir um .docx, aviso do que não aparece igual (macros, ActiveX, objetos do Word, imagens giradas ou soltas).
- ✅ Botões + e − nos cantos da tabela, como no FeatherNotes.
- ✅ Já existiam: "1." digitado vira lista; aceitar ou rejeitar todas as alterações.

## Falta

- ⬜ Seções de verdade na tela: papel, cabeçalho e rodapé por seção, numeração "continuar" ou "reiniciar em", e um painel mostrando isso. Hoje o motor só guarda as seções do .docx para devolvê-las.
- ⬜ Imagem solta do texto, com o texto contornando e a âncora visível (hoje aparece na linha).
- ⬜ Desfazer "Fixar na página" pelo clique (voltar a imagem ao texto): a figura presa à página ainda não é clicável.

## Fontes principais

- Tabela partida entre páginas: https://techcommunity.microsoft.com/discussions/word/word-table-splitting-across-pages/1362024/replies/4237485
- Editar dados do gráfico: https://learn.microsoft.com/en-us/answers/questions/4950875/2016-office-word-edit-chart-function-not-working
- Seleção do intervalo do gráfico: https://learn.microsoft.com/en-us/answers/questions/5295450/inconsistent-drag-to-select-chart-data-source
- Imagem que pula: https://word.tips.net/T001649_Stopping_Text_from_Jumping_Around.html
- Numeração que quebra: https://learn.microsoft.com/en-us/answers/questions/5543164/numbered-list-formatting-style-bug
- Campos (F9): https://askwoody.com/forums/topic/word-2010-table-of-contents-updates-but-list-of-figures-and-tables-dont
- Vincular ao anterior: https://techcommunity.microsoft.com/discussions/word/section-headers-and-footers---same-as-previous-/4460349/replies/4460596
- Recuperação automática: https://helpdeskgeek.com/?p=42419
