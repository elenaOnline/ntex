-- Text: getting into maths (mk, dm) and \text{} inside maths; LaTeX structure.
--
--   text(trig, output, origin)  fires by itself in text (outside maths), at the start of a word
--   dm has three versions, by what is on the line before it (line = …); the
--   output is computed (a function), because it is \[ … \] in .tex and $$ … $$ in .md.
--   { tab = true } needs Tab; { line = "start" } only at the start of a line;
--   { ft = "tex" } only in .tex files. Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local cond = require("snips.cond")
local math, text = h.math, h.text

-- dm: display maths on its own lines
local function block()
  local open, close = h.display_delims()
  return open .. "\n\t<>\n" .. close
end
-- dm after some text: the block starts on the next line
local function block_after_text()
  return "\n" .. block()
end
-- dm at the end of a list item: indented to line up with the item's text
local function block_in_list(_, line)
  local marker, ws = cond.list_marker(line)
  local indent = string.rep(" ", #(marker or "")) .. (ws or "")
  local open, close = h.display_delims()
  return "\n" .. indent .. open .. "\n" .. indent .. "\t<>\n" .. indent .. close
end

return h.group("Text and structure", {
  -- trigger       output                     origin
  text("mk",       [[$<>$]],                  "changed"),
  text("dm",       block,                     "changed", { line = "start", desc = "Display maths: \\[ … \\] (.tex) or $$ … $$ (.md)" }),
  text("dm",       block_after_text,          "yours",   { line = "after text", priority = 1, desc = "Display maths after some text: on the next line" }),
  text("dm",       block_in_list,             "changed", { line = "list item", priority = 2, desc = "Display math when in a list" }),

  math("text",     [[\text{<>}]],             "changed"),
  math('"',        [[\text{ <> }]],           "changed"),

  -- added from Gilles Castel's set (DECISIONS.md, "New from Castel": delete a line to veto it).
  -- All need Tab at the start of a line in a .tex file, so they can't fire while you write.
  text("table", [[
\begin{table}[<:htpb>]
	\centering
	\caption{<:caption>}
	\label{tab:<:label>}
	\begin{tabular}{<:c>}
	<>
	\end{tabular}
\end{table}]],                                "castel", { tab = true, line = "start", ft = "tex" }),
  text("fig", [[
\begin{figure}[<:htpb>]
	\centering
	\includegraphics[width=0.8\textwidth]{<>}
	\caption{<:caption>}
	\label{fig:<:label>}
\end{figure}]],                               "castel", { tab = true, line = "start", ft = "tex" }),
  text("desc", [[
\begin{description}
	\item[<>] <>
\end{description}]],                          "castel", { tab = true, line = "start", ft = "tex" }),
  text("pac",      [[\usepackage[<:options>]{<:package>}]], "castel", { tab = true, line = "start", ft = "tex" }),
  -- Castel's text environments, Tab-only (2026-10-01): typing "began",
  -- "items", "enum…", "align…" never expands; the word + Tab at a line start does.
  text("beg", [[
\begin{<1>}
	<2>
\end{<1>}]],                                 "castel", { tab = true, line = "start", ft = "tex", desc = "begin/end, name typed once" }),
  text("enum", [[
\begin{enumerate}
	\item <>
\end{enumerate}]],                            "castel", { tab = true, line = "start", ft = "tex" }),
  text("item", [[
\begin{itemize}
	\item <>
\end{itemize}]],                              "castel", { tab = true, line = "start", ft = "tex" }),
  text("ali", [[
\begin{align*}
	<>
\end{align*}]],                               "castel", { tab = true, line = "start", ft = "tex", desc = "align* (Castel's trailing '.' dropped)" }),
  text("plot", [[
\begin{figure}[<>]
	\centering
	\begin{tikzpicture}
		\begin{axis}[
			xmin= <2:-10>, xmax= <3:10>,
			ymin= <4:-10>, ymax = <5:10>,
			axis lines = middle,
		]
			\addplot[domain=<2>:<3>, samples=<6:100>]{<7>};
		\end{axis}
	\end{tikzpicture}
	\caption{<8>}
	\label{<9>}
\end{figure}]],                               "castel", { tab = true, line = "start", ft = "tex", desc = "Plot (needs \\usepackage{pgfplots})" }),
  text("bigfun", [[
\begin{align*}
	<1>: <2> &\longrightarrow <3> \\
	<4> &\longmapsto <1>(<4>) = <>
\end{align*}]],                               "castel", { tab = true, line = "start", ft = "tex", desc = "A function with domain and rule" }),
})
