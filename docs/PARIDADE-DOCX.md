# Paridade do `.docx` (ida e volta `.fdoz` ↔ `.docx`)

> Levantamento de **28/10/2026**, status **MEDIDO**, não deduzido. Gerado por `crates/featherdocs-io/examples/paridade_docx.rs`; não editar à mão.

Cada item é um `.docx` mínimo montado no exemplo (XML à mão dentro de um ZIP), com o texto `ZQX` no lugar do recurso. As três medidas saem do modelo (`featherdocs-core/src/modelo.rs`), não de ler o código:

- **Lê** — `docx::ler` põe o efeito no modelo?
- **Grava** — o modelo vai para `docx::gravar` e o elemento aparece no XML de saída? O modelo é o lido; quando o leitor não preenche um campo que o modelo tem, o exemplo monta o modelo direto (itens com † na tabela).
- **Volta igual** — `.docx` → modelo → `.fdoz` (gravar e ler) → `.docx` → modelo dá o mesmo resultado para aquele recurso? (Com † e Lê `[ ]`, parte do modelo montado.) A volta compara com a primeira leitura: um `[~]` em Lê pode ter `[x]` na volta (o que entrou no modelo não se perde mais).

Legenda: `[x]` completo · `[~]` parcial (a observação diz o que perde) · `[ ]` falta.

**Como refazer** (a partir de `crates/featherdocs-io`, sob limite de memória):

```bash
systemd-run --user --scope -q -p MemoryMax=8G cargo run --example paridade_docx -j 6 -- --saida ../../docs/PARIDADE-DOCX.md
```

`--detalhe` acrescenta o que cada medida viu. Para melhorar um item: mexa no leitor/gravador/modelo, rode de novo e confira a marca. Se o critério de um item estiver fraco, acerte-o no exemplo (a função `le` e os padrões `grava`).

## Resumo por seção

| Seção | Itens | Lê (x / ~ / falta) | Grava (x / ~ / falta) | Volta (x / ~ / falta) |
|---|---|---|---|---|
| 1. Caractere | 27 | 27 / 0 / 0 | 27 / 0 / 0 | 27 / 0 / 0 |
| 2. Parágrafo | 24 | 24 / 0 / 0 | 24 / 0 / 0 | 24 / 0 / 0 |
| 3. Estilos | 12 | 12 / 0 / 0 | 12 / 0 / 0 | 12 / 0 / 0 |
| 4. Listas | 15 | 15 / 0 / 0 | 15 / 0 / 0 | 15 / 0 / 0 |
| 5. Tabelas | 21 | 21 / 0 / 0 | 21 / 0 / 0 | 21 / 0 / 0 |
| 6. Imagens | 9 | 9 / 0 / 0 | 9 / 0 / 0 | 9 / 0 / 0 |
| 7. Formas, caixas de texto e objetos | 9 | 9 / 0 / 0 | 9 / 0 / 0 | 9 / 0 / 0 |
| 8. Seções e página | 21 | 21 / 0 / 0 | 21 / 0 / 0 | 21 / 0 / 0 |
| 9. Cabeçalho e rodapé | 10 | 10 / 0 / 0 | 10 / 0 / 0 | 10 / 0 / 0 |
| 10. Notas de rodapé e de fim | 6 | 6 / 0 / 0 | 6 / 0 / 0 | 6 / 0 / 0 |
| 11. Comentários | 6 | 6 / 0 / 0 | 6 / 0 / 0 | 6 / 0 / 0 |
| 12. Alterações controladas | 8 | 8 / 0 / 0 | 8 / 0 / 0 | 8 / 0 / 0 |
| 13. Campos | 9 | 9 / 0 / 0 | 9 / 0 / 0 | 9 / 0 / 0 |
| 14. Sumário, legendas, indicadores e links | 15 | 15 / 0 / 0 | 15 / 0 / 0 | 15 / 0 / 0 |
| 15. Fórmulas (OMML) | 4 | 4 / 0 / 0 | 4 / 0 / 0 | 4 / 0 / 0 |
| 16. Documento, idioma e marca d'água | 13 | 13 / 0 / 0 | 13 / 0 / 0 | 13 / 0 / 0 |
| **Total** | **209** | **209 / 0 / 0** | **209 / 0 / 0** | **209 / 0 / 0** |

## 1. Caractere

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Negrito | [x] | [x] | [x] |  |
| Itálico | [x] | [x] | [x] |  |
| Sublinhado simples | [x] | [x] | [x] |  |
| Sublinhado duplo | [x] | [x] | [x] |  |
| Sublinhado ondulado/pontilhado (outros tipos) | [x] | [x] | [x] |  |
| Cor do sublinhado | [x] | [x] | [x] |  |
| Tachado | [x] | [x] | [x] |  |
| Tachado duplo | [x] | [x] | [x] |  |
| Fonte (família) | [x] | [x] | [x] |  |
| Fonte sem serifa qualquer (Calibri) | [x] | [x] | [x] |  |
| Fonte monoespaçada (Consolas) | [x] | [x] | [x] |  |
| Fonte de tema (majorHAnsi) | [x] | [x] | [x] |  |
| Tamanho | [x] | [x] | [x] |  |
| Cor | [x] | [x] | [x] |  |
| Cor de tema (accent1) | [x] | [x] | [x] |  |
| Realce (marca-texto) | [x] | [x] | [x] |  |
| Sombreamento do caractere | [x] | [x] | [x] |  |
| Sobrescrito | [x] | [x] | [x] |  |
| Subscrito | [x] | [x] | [x] |  |
| Posição elevada/rebaixada (w:position) | [x] | [x] | [x] |  |
| Versalete | [x] | [x] | [x] |  |
| Todas em maiúsculas | [x] | [x] | [x] |  |
| Espaçamento entre letras | [x] | [x] | [x] |  |
| Escala horizontal | [x] | [x] | [x] |  |
| Kerning | [x] | [x] | [x] |  |
| Texto oculto | [x] | [x] | [x] |  |
| Efeitos (contorno, sombra, relevo) | [x] | [x] | [x] |  |

## 2. Parágrafo

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Alinhamento centralizado | [x] | [x] | [x] |  |
| Alinhamento à direita | [x] | [x] | [x] |  |
| Justificado | [x] | [x] | [x] |  |
| Justificado distribuído | [x] | [x] | [x] |  |
| Recuos esquerdo e direito | [x] | [x] | [x] |  |
| Recuo da primeira linha | [x] | [x] | [x] |  |
| Recuo deslocado (pendente) | [x] | [x] | [x] |  |
| Espaço antes e depois | [x] | [x] | [x] |  |
| Espaço automático antes/depois | [x] | [x] | [x] |  |
| Entrelinha múltipla (1,5) | [x] | [x] | [x] |  |
| Entrelinha exata | [x] | [x] | [x] |  |
| Entrelinha "pelo menos" | [x] | [x] | [x] |  |
| Não somar espaço entre parágrafos do mesmo estilo | [x] | [x] | [x] |  |
| Bordas do parágrafo (lados e cor) | [x] | [x] | [x] |  |
| Bordas do parágrafo (tipo e espessura) | [x] | [x] | [x] |  |
| Sombreamento do parágrafo | [x] | [x] | [x] |  |
| Manter com o próximo | [x] | [x] | [x] |  |
| Manter linhas juntas | [x] | [x] | [x] |  |
| Controle de linhas órfãs/viúvas (desligado) | [x] | [x] | [x] |  |
| Quebra de página antes | [x] | [x] | [x] |  |
| Tabulações personalizadas (posição, alinhamento, preenchimento) | [x] | [x] | [x] |  |
| Nível de tópico (outlineLvl) num parágrafo comum | [x] | [x] | [x] |  |
| Letra capitular | [x] | [x] | [x] |  |
| Quebra de linha (Shift+Enter) | [x] | [x] | [x] |  |

## 3. Estilos

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Estilo de parágrafo personalizado (formatação do estilo) | [x] | [x] | [x] |  |
| Nome do estilo personalizado na lista de estilos | [x] | [x] | [x] |  |
| Estilo de caractere (rStyle) | [x] | [x] | [x] |  |
| Herança de estilos (basedOn) | [x] | [x] | [x] |  |
| Padrão do documento (docDefaults: fonte e tamanho) | [x] | [x] | [x] |  |
| Estilo Normal modificado | [x] | [x] | [x] |  |
| Títulos internos (Heading 1–9) | [x] | [x] | [x] |  |
| Aparência do estilo de título (cor e tamanho do styles.xml) | [x] | [x] | [x] |  |
| Estilo Título (Title) e Subtítulo | [x] | [x] | [x] |  |
| Estilo Citação (Quote) | [x] | [x] | [x] |  |
| Estilo com nome criado no FeatherDocs (FDEstilo) | [x] | [x] | [x] |  |
| Estilo de tabela (tblStyle) | [x] | [x] | [x] |  |

## 4. Listas

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Lista com marcadores | [x] | [x] | [x] |  |
| Lista numerada | [x] | [x] | [x] |  |
| Multinível (subitem) | [x] | [x] | [x] |  |
| Formato do número por nível (a), i.) | [x] | [x] | [x] |  |
| Letra ou romano no primeiro nível | [x] | [x] | [x] |  |
| Texto do número personalizado ("Passo 1:") | [x] | [x] | [x] |  |
| Início da numeração em outro número | [x] | [x] | [x] |  |
| Reinício da numeração (startOverride) | [x] | [x] | [x] |  |
| Continuar a numeração depois de um parágrafo | [x] | [x] | [x] |  |
| Marcador de outro caractere (➢) | [x] | [x] | [x] |  |
| Marcador de símbolo (Wingdings) | [x] | [x] | [x] |  |
| Lista de tarefas (☐) | [x] | [x] | [x] |  |
| Estilo de lista (numStyleLink) | [x] | [x] | [x] |  |
| Numeração vinda do estilo de parágrafo (List Number) | [x] | [x] | [x] |  |
| Recuo próprio do item | [x] | [x] | [x] |  |

## 5. Tabelas

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Tabela simples | [x] | [x] | [x] |  |
| Linha de cabeçalho repetida | [x] | [x] | [x] |  |
| Mescla horizontal (gridSpan) † | [x] | [x] | [x] |  |
| Mescla vertical (vMerge) | [x] | [x] | [x] |  |
| Larguras das colunas | [x] | [x] | [x] |  |
| Largura da tabela (porcentagem) | [x] | [x] | [x] |  |
| Alinhamento da tabela na página | [x] | [x] | [x] |  |
| Autoajuste / layout fixo | [x] | [x] | [x] |  |
| Bordas da tabela (tblBorders) | [x] | [x] | [x] |  |
| Bordas por célula (tcBorders) | [x] | [x] | [x] |  |
| Sombreamento de célula | [x] | [x] | [x] |  |
| Alinhamento vertical da célula | [x] | [x] | [x] |  |
| Alinhamento do texto na célula | [x] | [x] | [x] |  |
| Altura da linha | [x] | [x] | [x] |  |
| Direção do texto na célula (vertical) | [x] | [x] | [x] |  |
| Formatação de caractere na célula | [x] | [x] | [x] |  |
| Vários parágrafos numa célula | [x] | [x] | [x] |  |
| Lista dentro de célula | [x] | [x] | [x] |  |
| Imagem dentro de célula | [x] | [x] | [x] |  |
| Tabela aninhada | [x] | [x] | [x] |  |
| Margens da célula (tcMar) | [x] | [x] | [x] |  |

## 6. Imagens

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Imagem em linha (embutida) | [x] | [x] | [x] |  |
| Tamanho da imagem | [x] | [x] | [x] |  |
| Texto alternativo (descr) | [x] | [x] | [x] |  |
| Imagem flutuante (âncora, posição, quebra automática) | [x] | [x] | [x] |  |
| Imagem no meio do texto † | [x] | [x] | [x] |  |
| Recorte (srcRect) | [x] | [x] | [x] |  |
| Rotação e espelhamento | [x] | [x] | [x] |  |
| Imagem ligada (endereço da web) | [x] | [x] | [x] |  |
| Imagem VML antiga (w:pict/v:imagedata) | [x] | [x] | [x] |  |

## 7. Formas, caixas de texto e objetos

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Caixa de texto | [x] | [x] | [x] |  |
| Forma (retângulo, seta…) | [x] | [x] | [x] |  |
| Gráfico (c:chart) | [x] | [x] | [x] |  |
| SmartArt | [x] | [x] | [x] |  |
| Objeto OLE (planilha incorporada) | [x] | [x] | [x] |  |
| Controle de conteúdo de texto (sdt) | [x] | [x] | [x] |  |
| Caixa de seleção (sdt checkbox) | [x] | [x] | [x] |  |
| Lista suspensa (sdt dropDownList) | [x] | [x] | [x] |  |
| Seletor de data (sdt date) | [x] | [x] | [x] |  |

## 8. Seções e página

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Tamanho do papel (A4) | [x] | [x] | [x] |  |
| Papel de outro tamanho (A5) | [x] | [x] | [x] |  |
| Orientação paisagem | [x] | [x] | [x] |  |
| Margens | [x] | [x] | [x] |  |
| Medianiz | [x] | [x] | [x] |  |
| Distância do cabeçalho e do rodapé à borda | [x] | [x] | [x] |  |
| Colunas de texto | [x] | [x] | [x] |  |
| Quebra de seção (próxima página) | [x] | [x] | [x] |  |
| Quebra de seção contínua | [x] | [x] | [x] |  |
| Quebra de seção em página ímpar/par | [x] | [x] | [x] |  |
| Várias seções com papel próprio (retrato e paisagem) | [x] | [x] | [x] |  |
| Alinhamento vertical da página | [x] | [x] | [x] |  |
| Número da primeira página (pgNumType start) † | [x] | [x] | [x] |  |
| Formato do número de página (romano) | [x] | [x] | [x] |  |
| Quebra de página manual | [x] | [x] | [x] |  |
| Quebra de coluna | [x] | [x] | [x] |  |
| Cor da página | [x] | [x] | [x] |  |
| Borda da página | [x] | [x] | [x] |  |
| Borda da página com arte ou traço duplo | [x] | [x] | [x] |  |
| Números de linha (contínuos) | [x] | [x] | [x] |  |
| Números de linha (por página, de 5 em 5) | [x] | [x] | [x] |  |

## 9. Cabeçalho e rodapé

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Cabeçalho com texto | [x] | [x] | [x] |  |
| Rodapé com número de página (PAGE) | [x] | [x] | [x] |  |
| "Página X de Y" (PAGE e NUMPAGES) | [x] | [x] | [x] |  |
| Três posições com tabulação (esquerda, centro, direita) | [x] | [x] | [x] |  |
| Letra do cabeçalho (negrito, tamanho, cor) † | [x] | [x] | [x] |  |
| Imagem no cabeçalho (logo) † | [x] | [x] | [x] |  |
| Tabela no cabeçalho | [x] | [x] | [x] |  |
| Primeira página diferente † | [x] | [x] | [x] |  |
| Páginas pares e ímpares diferentes | [x] | [x] | [x] |  |
| Cabeçalho próprio em cada seção | [x] | [x] | [x] |  |

## 10. Notas de rodapé e de fim

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Nota de rodapé | [x] | [x] | [x] |  |
| Nota de fim | [x] | [x] | [x] |  |
| Formatação no texto da nota | [x] | [x] | [x] |  |
| Vários parágrafos numa nota | [x] | [x] | [x] |  |
| Formato do número das notas (i, ii, iii) † | [x] | [x] | [x] |  |
| Recomeçar a numeração das notas a cada página | [x] | [x] | [x] |  |

## 11. Comentários

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Comentário num trecho | [x] | [x] | [x] |  |
| Autor e data do comentário | [x] | [x] | [x] |  |
| Iniciais do autor | [x] | [x] | [x] |  |
| Resposta a comentário | [x] | [x] | [x] |  |
| Comentário resolvido | [x] | [x] | [x] |  |
| Formatação dentro do comentário | [x] | [x] | [x] |  |

## 12. Alterações controladas

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Inserção | [x] | [x] | [x] |  |
| Exclusão | [x] | [x] | [x] |  |
| Formatação alterada (rPrChange) | [x] | [x] | [x] |  |
| Parágrafo alterado (pPrChange) | [x] | [x] | [x] |  |
| Texto movido (moveFrom/moveTo) | [x] | [x] | [x] |  |
| Marca de parágrafo inserido | [x] | [x] | [x] |  |
| Controlar alterações ligado (trackRevisions) | [x] | [x] | [x] |  |
| Linha de tabela inserida | [x] | [x] | [x] |  |

## 13. Campos

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| PAGE no corpo (campo complexo) | [x] | [x] | [x] |  |
| NUMPAGES (campo simples) | [x] | [x] | [x] |  |
| DATE com formato | [x] | [x] | [x] |  |
| AUTHOR e TITLE | [x] | [x] | [x] |  |
| FILENAME | [x] | [x] | [x] |  |
| HYPERLINK como campo | [x] | [x] | [x] |  |
| MERGEFIELD (mala direta) | [x] | [x] | [x] |  |
| Campo de cálculo (= SUM(ABOVE), IF) | [x] | [x] | [x] |  |
| Resultado do campo guardado (campo desconhecido) | [x] | [x] | [x] |  |

## 14. Sumário, legendas, indicadores e links

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Sumário (campo TOC) | [x] | [x] | [x] |  |
| Sumário com outros níveis e sem números de página | [x] | [x] | [x] |  |
| Entradas do sumário guardadas (texto e página) | [x] | [x] | [x] |  |
| Lista de figuras (TOC \c) | [x] | [x] | [x] |  |
| Legenda de figura (SEQ Figura) | [x] | [x] | [x] |  |
| Legenda de tabela (SEQ Tabela) | [x] | [x] | [x] |  |
| Legenda com rótulo próprio (SEQ Quadro) | [x] | [x] | [x] |  |
| Indicador (bookmark) † | [x] | [x] | [x] |  |
| Referência cruzada (REF) | [x] | [x] | [x] |  |
| Referência à página (PAGEREF) | [x] | [x] | [x] |  |
| Hiperlink externo | [x] | [x] | [x] |  |
| Hiperlink interno (para indicador) | [x] | [x] | [x] |  |
| Dica de tela do link (tooltip) | [x] | [x] | [x] |  |
| Índice remissivo (XE e INDEX) | [x] | [x] | [x] |  |
| Citação e fontes (CITATION e b:Sources) | [x] | [x] | [x] |  |

## 15. Fórmulas (OMML)

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Fórmula em linha | [x] | [x] | [x] |  |
| Fórmula em destaque (oMathPara) | [x] | [x] | [x] |  |
| Matriz, somatório e raiz | [x] | [x] | [x] |  |
| Fórmula numerada (equação com SEQ) | [x] | [x] | [x] |  |

## 16. Documento, idioma e marca d'água

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Título e autor (docProps/core.xml) | [x] | [x] | [x] |  |
| Assunto, palavras-chave, descrição, datas (core.xml) | [x] | [x] | [x] |  |
| Propriedades do aplicativo (app.xml: empresa, gerente) | [x] | [x] | [x] |  |
| Propriedades personalizadas (custom.xml) | [x] | [x] | [x] |  |
| Marca d'água de texto † | [x] | [x] | [x] |  |
| Proteção do documento (só leitura) | [x] | [x] | [x] |  |
| Fontes embutidas (embedTrueTypeFonts) | [x] | [x] | [x] |  |
| Idioma do texto (w:lang) | [x] | [x] | [x] |  |
| Idioma padrão do documento | [x] | [x] | [x] |  |
| Texto da direita para a esquerda (bidi/rtl) | [x] | [x] | [x] |  |
| Hifenização automática | [x] | [x] | [x] |  |
| Não hifenizar o parágrafo | [x] | [x] | [x] |  |
| Parada de tabulação padrão (defaultTabStop) | [x] | [x] | [x] |  |

## Resumo

**209 itens** em 16 seções.

| Coluna | Completo `[x]` | Parcial `[~]` | Falta `[ ]` |
|---|---|---|---|
| Lê | 209 | 0 | 0 |
| Grava | 209 | 0 | 0 |
| Volta igual | 209 | 0 | 0 |

Completos nas três colunas: **209 / 209**.

### As 10 lacunas mais importantes para o documento comum de trabalho

Em ordem de uso no dia a dia (estilos, tabelas, imagens, listas, cabeçalho/rodapé, seções, sumário); a ordem é a constante `PRIORIDADE` do exemplo, e entram as 10 primeiras que não estão completas nas três colunas.


## O que não está medido

- **Fidelidade visual** (quebras de linha e de página iguais às do Word, métricas de fonte): aqui só se mede se o recurso chega ao modelo e ao XML.
- **Arquivos reais**: os `.docx` são mínimos, feitos à mão; um documento do Word traz combinações (estilo + formatação direta + tema) que estes itens não cobrem. O próximo passo é rodar o mesmo critério sobre arquivos de usuário.
- **Tema** (`theme1.xml`): o leitor guarda o tema inteiro e o devolve, mas só entende as fontes (item "Fonte de tema"); as cores de tema não têm item que confira o vínculo além de "Cor de tema".
- **WordArt**, **tela de desenho e grupos de formas**, **vídeo online**, **macros** (`.docm`) e **assinatura digital**: sem item próprio; o leitor não abre essas partes.
- **Grava** só confere o elemento no XML, não se o Word o abre sem reclamar (o esquema completo do OOXML não é validado).
