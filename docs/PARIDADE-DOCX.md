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
| 3. Estilos | 12 | 11 / 0 / 1 | 11 / 0 / 1 | 11 / 0 / 1 |
| 4. Listas | 15 | 15 / 0 / 0 | 15 / 0 / 0 | 15 / 0 / 0 |
| 5. Tabelas | 21 | 17 / 3 / 1 | 17 / 3 / 1 | 20 / 0 / 1 |
| 6. Imagens | 9 | 3 / 2 / 4 | 3 / 1 / 5 | 5 / 0 / 4 |
| 7. Formas, caixas de texto e objetos | 9 | 0 / 9 / 0 | 8 / 1 / 0 | 9 / 0 / 0 |
| 8. Seções e página | 21 | 13 / 3 / 5 | 13 / 3 / 5 | 16 / 0 / 5 |
| 9. Cabeçalho e rodapé | 10 | 7 / 2 / 1 | 7 / 2 / 1 | 9 / 0 / 1 |
| 10. Notas de rodapé e de fim | 6 | 3 / 1 / 2 | 3 / 1 / 2 | 4 / 0 / 2 |
| 11. Comentários | 6 | 4 / 2 / 0 | 5 / 1 / 0 | 6 / 0 / 0 |
| 12. Alterações controladas | 8 | 3 / 0 / 5 | 3 / 0 / 5 | 3 / 0 / 5 |
| 13. Campos | 9 | 4 / 5 / 0 | 4 / 4 / 1 | 9 / 0 / 0 |
| 14. Sumário, legendas, indicadores e links | 15 | 9 / 5 / 1 | 9 / 5 / 1 | 14 / 0 / 1 |
| 15. Fórmulas (OMML) | 4 | 3 / 1 / 0 | 3 / 1 / 0 | 4 / 0 / 0 |
| 16. Documento, idioma e marca d'água | 13 | 9 / 0 / 4 | 9 / 0 / 4 | 9 / 0 / 4 |
| **Total** | **209** | **152 / 33 / 24** | **161 / 22 / 26** | **185 / 0 / 24** |

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
| Estilo de tabela (tblStyle) | [ ] | [ ] | [ ] | Lê: o modelo não tem estilo de tabela |

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
| Vários parágrafos numa célula | [~] | [~] | [x] | Lê: viram um só, separados por espaço: "um ZQX" · Grava: falta um</w:t></w:r></w:p><w:p> |
| Lista dentro de célula | [~] | [~] | [x] | Lê: o texto fica, o marcador some · Grava: falta </w:tcPr><w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr> |
| Imagem dentro de célula | [ ] | [ ] | [ ] | Lê: a imagem some (a célula guarda só texto) |
| Tabela aninhada | [~] | [~] | [x] | Lê: a tabela de dentro vira texto na célula de fora · Grava: falta </w:tcPr><w:tbl> |
| Margens da célula (tcMar) | [x] | [x] | [x] |  |

## 6. Imagens

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Imagem em linha (embutida) | [x] | [x] | [x] |  |
| Tamanho da imagem | [x] | [x] | [x] |  |
| Texto alternativo (descr) | [ ] | [ ] | [ ] | Lê: o modelo não tem texto alternativo |
| Imagem flutuante (âncora, posição, quebra automática) | [~] | [~] | [x] | Lê: vira imagem em linha num bloco próprio: perde posição e quebra do texto · Grava: falta <wp:anchor · <wp:wrapSquare |
| Imagem no meio do texto † | [~] | [ ] | [x] | Lê: sai do parágrafo e vira bloco próprio depois dele · Grava: falta antes </w:t></w:r><w:r><w:drawing><wp:inline |
| Recorte (srcRect) | [ ] | [ ] | [ ] | Lê: o modelo não tem recorte: a imagem volta inteira |
| Rotação e espelhamento | [ ] | [ ] | [ ] | Lê: o modelo não tem rotação |
| Imagem ligada (endereço da web) | [x] | [x] | [x] |  |
| Imagem VML antiga (w:pict/v:imagedata) | [ ] | [ ] | [ ] | Lê: 0 imagens |

## 7. Formas, caixas de texto e objetos

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Caixa de texto | [~] | [~] | [x] | Lê: o texto vira parágrafo solto, 2 vezes (lê a Choice e a Fallback); a caixa some · Grava: falta <wps:txbx · <w:txbxContent> |
| Forma (retângulo, seta…) | [~] | [x] | [x] | Lê: guardado na sacola (forma; partes: ) |
| Gráfico (c:chart) | [~] | [x] | [x] | Lê: guardado na sacola (gráfico; partes: word/charts/chart1.xml) |
| SmartArt | [~] | [x] | [x] | Lê: guardado na sacola (SmartArt; partes: ) |
| Objeto OLE (planilha incorporada) | [~] | [x] | [x] | Lê: guardado na sacola (objeto OLE (Excel.Sheet.12); partes: word/media/img.png, word/embeddings/planilha.xlsx) |
| Controle de conteúdo de texto (sdt) | [~] | [x] | [x] | Lê: guardado na sacola (o w:alias volta igual; o texto fica) |
| Caixa de seleção (sdt checkbox) | [~] | [x] | [x] | Lê: guardado na sacola (o w14:checkbox volta igual; o texto fica) |
| Lista suspensa (sdt dropDownList) | [~] | [x] | [x] | Lê: guardado na sacola (o w:dropDownList volta igual; o texto fica) |
| Seletor de data (sdt date) | [~] | [x] | [x] | Lê: guardado na sacola (o w:date volta igual; o texto fica) |

## 8. Seções e página

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Tamanho do papel (A4) | [x] | [x] | [x] |  |
| Papel de outro tamanho (A5) | [x] | [x] | [x] |  |
| Orientação paisagem | [x] | [x] | [x] |  |
| Margens | [x] | [x] | [x] |  |
| Medianiz | [ ] | [ ] | [ ] | Lê: o modelo não tem medianiz |
| Distância do cabeçalho e do rodapé à borda | [ ] | [ ] | [ ] | Lê: o modelo não tem; grava 708 fixo |
| Colunas de texto | [ ] | [ ] | [ ] | Lê: o modelo não tem número de colunas |
| Quebra de seção (próxima página) | [x] | [x] | [x] |  |
| Quebra de seção contínua | [x] | [x] | [x] |  |
| Quebra de seção em página ímpar/par | [x] | [x] | [x] |  |
| Várias seções com papel próprio (retrato e paisagem) | [~] | [~] | [x] | Lê: a quebra fica; o papel é um só no documento (841.9×595.3) · Grava: falta w:w="11906" w:h="16838"/><w:pgMar |
| Alinhamento vertical da página | [x] | [x] | [x] |  |
| Número da primeira página (pgNumType start) † | [ ] | [ ] | [ ] | Lê: começa em 1 · Grava: falta <w:pgNumType w:start="5" · Volta: perde no .docx: «começa em 5» → «começa em 1» |
| Formato do número de página (romano) | [ ] | [ ] | [ ] | Lê: o modelo não tem formato por seção |
| Quebra de página manual | [x] | [x] | [x] |  |
| Quebra de coluna | [x] | [x] | [x] |  |
| Cor da página | [x] | [x] | [x] |  |
| Borda da página | [x] | [x] | [x] |  |
| Borda da página com arte ou traço duplo | [~] | [~] | [x] | Lê: vira traço simples nos quatro lados · Grava: falta <w:top w:val="double" |
| Números de linha (contínuos) | [x] | [x] | [x] |  |
| Números de linha (por página, de 5 em 5) | [~] | [~] | [x] | Lê: por página, mas de 1 em 1 (o passo de 5 some) · Grava: falta w:countBy="5" w:restart="newPage" |

## 9. Cabeçalho e rodapé

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Cabeçalho com texto | [x] | [x] | [x] |  |
| Rodapé com número de página (PAGE) | [x] | [x] | [x] |  |
| "Página X de Y" (PAGE e NUMPAGES) | [x] | [x] | [x] |  |
| Três posições com tabulação (esquerda, centro, direita) | [x] | [x] | [x] |  |
| Letra do cabeçalho (negrito, tamanho, cor) † | [x] | [x] | [x] |  |
| Imagem no cabeçalho (logo) † | [x] | [x] | [x] |  |
| Tabela no cabeçalho | [~] | [~] | [x] | Lê: a tabela vira texto corrido: "Logo ZQX" · Grava: falta header><w:tbl> |
| Primeira página diferente † | [x] | [x] | [x] |  |
| Páginas pares e ímpares diferentes | [ ] | [ ] | [ ] | Lê: o modelo não tem cabeçalho de página par |
| Cabeçalho próprio em cada seção | [~] | [~] | [x] | Lê: só um cabeçalho no documento: "SEGUNDA" · Grava: falta header>ZQX |

## 10. Notas de rodapé e de fim

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Nota de rodapé | [x] | [x] | [x] |  |
| Nota de fim | [x] | [x] | [x] |  |
| Formatação no texto da nota | [x] | [x] | [x] |  |
| Vários parágrafos numa nota | [~] | [~] | [x] | Lê: viram um parágrafo só: "um ZQX" · Grava: falta footnotes>um</w:t></w:r></w:p><w:p> |
| Formato do número das notas (i, ii, iii) † | [ ] | [ ] | [ ] | Lê: formato 0 · Grava: falta <w:footnotePr><w:numFmt w:val="lowerRoman" · Volta: perde no .docx: «formato 2» → «formato 0» |
| Recomeçar a numeração das notas a cada página | [ ] | [ ] | [ ] | Lê: o modelo não tem |

## 11. Comentários

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Comentário num trecho | [x] | [x] | [x] |  |
| Autor e data do comentário | [x] | [x] | [x] |  |
| Iniciais do autor | [~] | [x] | [x] | Lê: refeitas do nome |
| Resposta a comentário | [x] | [x] | [x] |  |
| Comentário resolvido | [x] | [x] | [x] |  |
| Formatação dentro do comentário | [~] | [~] | [x] | Lê: o texto fica, o negrito some (o comentário é texto puro) · Grava: falta comments><w:b/> |

## 12. Alterações controladas

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Inserção | [x] | [x] | [x] |  |
| Exclusão | [x] | [x] | [x] |  |
| Formatação alterada (rPrChange) | [ ] | [ ] | [ ] | Lê: fica a formatação nova, sem a marca de alteração |
| Parágrafo alterado (pPrChange) | [ ] | [ ] | [ ] | Lê: fica o parágrafo novo, sem a marca de alteração |
| Texto movido (moveFrom/moveTo) | [ ] | [ ] | [ ] | Lê: o texto aparece 2 vezes, 0 com marca |
| Marca de parágrafo inserido | [ ] | [ ] | [ ] | Lê: o fim de parágrafo inserido não tem marca no modelo |
| Controlar alterações ligado (trackRevisions) | [x] | [x] | [x] |  |
| Linha de tabela inserida | [ ] | [ ] | [ ] | Lê: a linha fica, sem a marca de alteração |

## 13. Campos

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| PAGE no corpo (campo complexo) | [x] | [x] | [x] |  |
| NUMPAGES (campo simples) | [x] | [x] | [x] |  |
| DATE com formato | [~] | [~] | [x] | Lê: fica o campo; o formato vira dd/MM/yyyy · Grava: falta DATE \@ &quot;d 'de' MMMM |
| AUTHOR e TITLE | [x] | [x] | [x] |  |
| FILENAME | [x] | [x] | [x] |  |
| HYPERLINK como campo | [~] | [ ] | [x] | Lê: fica o texto, sem o link · Grava: falta <w:hyperlink |
| MERGEFIELD (mala direta) | [~] | [~] | [x] | Lê: fica o texto «Nome», sem o campo · Grava: falta MERGEFIELD |
| Campo de cálculo (= SUM(ABOVE), IF) | [~] | [~] | [x] | Lê: fica o resultado como texto · Grava: falta = 2+3 |
| Resultado do campo guardado (campo desconhecido) | [~] | [~] | [x] | Lê: fica o texto do resultado, sem o campo · Grava: falta DOCPROPERTY |

## 14. Sumário, legendas, indicadores e links

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Sumário (campo TOC) | [x] | [x] | [x] |  |
| Sumário com outros níveis e sem números de página | [x] | [x] | [x] |  |
| Entradas do sumário guardadas (texto e página) | [~] | [~] | [x] | Lê: o sumário é refeito dos títulos; as entradas guardadas pelo Word somem · Grava: falta <w:pStyle w:val="TOC1" |
| Lista de figuras (TOC \c) | [x] | [x] | [x] |  |
| Legenda de figura (SEQ Figura) | [x] | [x] | [x] |  |
| Legenda de tabela (SEQ Tabela) | [x] | [x] | [x] |  |
| Legenda com rótulo próprio (SEQ Quadro) | [~] | [~] | [x] | Lê: vira legenda de figura · Grava: falta SEQ Quadro |
| Indicador (bookmark) † | [ ] | [ ] | [ ] | Lê: 0 marcadores · Grava: falta w:bookmarkStart · w:name="sec1" · Volta: perde no .docx: «1 marcadores» → «0 marcadores» |
| Referência cruzada (REF) | [~] | [~] | [x] | Lê: fica o texto do resultado, sem a referência · Grava: falta REF sec1 |
| Referência à página (PAGEREF) | [~] | [~] | [x] | Lê: fica o número como texto, sem a referência · Grava: falta PAGEREF sec1 |
| Hiperlink externo | [x] | [x] | [x] |  |
| Hiperlink interno (para indicador) | [x] | [x] | [x] |  |
| Dica de tela do link (tooltip) | [~] | [~] | [x] | Lê: fica o link, a dica some · Grava: falta w:tooltip="Abrir o site" |
| Índice remissivo (XE e INDEX) | [x] | [x] | [x] |  |
| Citação e fontes (CITATION e b:Sources) | [x] | [x] | [x] |  |

## 15. Fórmulas (OMML)

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Fórmula em linha | [x] | [x] | [x] |  |
| Fórmula em destaque (oMathPara) | [x] | [x] | [x] |  |
| Matriz, somatório e raiz | [x] | [x] | [x] |  |
| Fórmula numerada (equação com SEQ) | [~] | [~] | [x] | Lê: vira legenda de equação, mas a fórmula antes do SEQ some · Grava: falta <m:oMath> |

## 16. Documento, idioma e marca d'água

| Recurso | Lê | Grava | Volta igual | Observação |
|---|---|---|---|---|
| Título e autor (docProps/core.xml) | [x] | [x] | [x] |  |
| Assunto, palavras-chave, descrição, datas (core.xml) | [x] | [x] | [x] |  |
| Propriedades do aplicativo (app.xml: empresa, gerente) | [x] | [x] | [x] |  |
| Propriedades personalizadas (custom.xml) | [x] | [x] | [x] |  |
| Marca d'água de texto † | [x] | [x] | [x] |  |
| Proteção do documento (só leitura) | [x] | [x] | [x] |  |
| Fontes embutidas (embedTrueTypeFonts) | [ ] | [ ] | [ ] | Lê: o modelo não tem |
| Idioma do texto (w:lang) | [ ] | [ ] | [ ] | Lê: o modelo não tem idioma por trecho |
| Idioma padrão do documento | [x] | [x] | [x] |  |
| Texto da direita para a esquerda (bidi/rtl) | [ ] | [ ] | [ ] | Lê: o modelo não tem direção do texto |
| Hifenização automática | [x] | [x] | [x] |  |
| Não hifenizar o parágrafo | [ ] | [ ] | [ ] | Lê: o modelo não tem |
| Parada de tabulação padrão (defaultTabStop) | [x] | [x] | [x] |  |

## Resumo

**209 itens** em 16 seções.

| Coluna | Completo `[x]` | Parcial `[~]` | Falta `[ ]` |
|---|---|---|---|
| Lê | 152 | 33 | 24 |
| Grava | 161 | 22 | 26 |
| Volta igual | 185 | 0 | 24 |

Completos nas três colunas: **152 / 209**.

### As 10 lacunas mais importantes para o documento comum de trabalho

Em ordem de uso no dia a dia (estilos, tabelas, imagens, listas, cabeçalho/rodapé, seções, sumário); a ordem é a constante `PRIORIDADE` do exemplo, e entram as 10 primeiras que não estão completas nas três colunas.

1. **Imagem flutuante (âncora, posição, quebra automática)** (6. Imagens) — Lê [~] · Grava [~] · Volta [x] — vira imagem em linha num bloco próprio: perde posição e quebra do texto
2. **Cabeçalho próprio em cada seção** (9. Cabeçalho e rodapé) — Lê [~] · Grava [~] · Volta [x] — só um cabeçalho no documento: "SEGUNDA"
3. **Várias seções com papel próprio (retrato e paisagem)** (8. Seções e página) — Lê [~] · Grava [~] · Volta [x] — a quebra fica; o papel é um só no documento (841.9×595.3)
4. **Texto alternativo (descr)** (6. Imagens) — Lê [ ] · Grava [ ] · Volta [ ] — o modelo não tem texto alternativo
5. **Imagem dentro de célula** (5. Tabelas) — Lê [ ] · Grava [ ] · Volta [ ] — a imagem some (a célula guarda só texto)
6. **Colunas de texto** (8. Seções e página) — Lê [ ] · Grava [ ] · Volta [ ] — o modelo não tem número de colunas
7. **Indicador (bookmark)** (14. Sumário, legendas, indicadores e links) — Lê [ ] · Grava [ ] · Volta [ ] — 0 marcadores
8. **Referência cruzada (REF)** (14. Sumário, legendas, indicadores e links) — Lê [~] · Grava [~] · Volta [x] — fica o texto do resultado, sem a referência
9. **Caixa de texto** (7. Formas, caixas de texto e objetos) — Lê [~] · Grava [~] · Volta [x] — o texto vira parágrafo solto, 2 vezes (lê a Choice e a Fallback); a caixa some
10. **Páginas pares e ímpares diferentes** (9. Cabeçalho e rodapé) — Lê [ ] · Grava [ ] · Volta [ ] — o modelo não tem cabeçalho de página par

## O que não está medido

- **Fidelidade visual** (quebras de linha e de página iguais às do Word, métricas de fonte): aqui só se mede se o recurso chega ao modelo e ao XML.
- **Arquivos reais**: os `.docx` são mínimos, feitos à mão; um documento do Word traz combinações (estilo + formatação direta + tema) que estes itens não cobrem. O próximo passo é rodar o mesmo critério sobre arquivos de usuário.
- **Tema** (`theme1.xml`): o leitor guarda o tema inteiro e o devolve, mas só entende as fontes (item "Fonte de tema"); as cores de tema não têm item que confira o vínculo além de "Cor de tema".
- **WordArt**, **tela de desenho e grupos de formas**, **vídeo online**, **macros** (`.docm`) e **assinatura digital**: sem item próprio; o leitor não abre essas partes.
- **Grava** só confere o elemento no XML, não se o Word o abre sem reclamar (o esquema completo do OOXML não é validado).
