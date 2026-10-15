// LaTeX layout & formatting standards (amsmath practice). Edit freely.
//
// Each section is { id, title, body } where body is HTML built with helpers:
//   src`…`    a block of LaTeX source (shown as code)
//   tex`…`    inline maths, rendered by KaTeX at build time
//   show(source, render?, note?)  "You write / You get" pair; render defaults
//             to source and is KaTeX display maths (use aligned, not align)
//   c`…`      inline code
// Write the LaTeX exactly as you would in a .tex file (these are String.raw).

export function latexStyle({ src, tex, show, c, template }) {
  return [
    {
      id: "skeleton",
      title: "Document skeleton",
      body: `
<p>New <code>.tex</code> files start from <code>~/.config/texnvim/templates/article.tex</code> (edit it to change every future file). This is it, exactly:</p>
${template}
<ul>
<li>Packages go in the preamble (before ${c`\begin{document}`}); ${c`hyperref`} goes last, ${c`cleveref`} after it.</li>
<li>tectonic reads UTF-8 directly, so ${c`\usepackage[utf8]{inputenc}`} is not needed (é, ü, “ ” work as typed).</li>
<li>One sentence per line in the source makes changes and undo easier; LaTeX joins lines into paragraphs. A <em>blank line</em> starts a new paragraph; never use ${c`\\`} for that.</li>
</ul>`,
    },
    {
      id: "sectioning",
      title: "Sectioning",
      body: `
${src`\section{Compact operators}\label{sec:compact}
\subsection{Definitions}\label{sec:compact-defs}
\subsubsection*{A remark}   % * = no number, not in the contents
\paragraph{Notation.} Run-in heading, for small steps.`}
<ul>
<li>Never number headings by hand; LaTeX numbers them and ${c`\ref`} follows.</li>
<li>Sentence case (“Compact operators”, not “Compact Operators”) is the common house style in maths; pick one and keep it.</li>
<li>${c`\tableofcontents`} after ${c`\maketitle`} lists the numbered sections. (Plain LaTeX needs a second run for it; tectonic reruns by itself, so one compile is enough.)</li>
</ul>`,
    },
    {
      id: "math-modes",
      title: "Inline and display maths",
      body: `
<table class="compare"><thead><tr><th scope="col">Use</th><th scope="col">For</th></tr></thead><tbody>
<tr><td>${c`$x^2$`} or ${c`\(x^2\)`}</td><td>maths inside a sentence. Both are correct; ${c`mk`} gives ${c`$…$`}. ${c`\(…\)`} gives clearer error messages.</td></tr>
<tr><td>${c`\[ … \]`}</td><td>one displayed, <em>unnumbered</em> formula (same as ${c`equation*`})</td></tr>
<tr><td>${c`\begin{equation} … \end{equation}`}</td><td>one displayed, <em>numbered</em> formula you refer to</td></tr>
<tr><td class="bad">${c`$$ … $$`}</td><td><strong>Never in a .tex file.</strong> It is plain TeX: inconsistent spacing, breaks ${c`fleqn`} and amsmath. Your ${c`dm`} snippet already knows: in .tex it types ${c`\[ … \]`}, in Markdown (.md, Obsidian, Superset) ${c`$$ … $$`}, which is right there.</td></tr>
<tr><td class="bad">${c`eqnarray`}</td><td><strong>Never.</strong> Wrong spacing around ${c`=`}. Use ${c`align`}.</td></tr>
</tbody></table>
<p>Display maths belongs to the sentence around it: no blank line before or after it (a blank line starts a new paragraph and adds space), and no blank line inside it (that is an error).</p>`,
    },
    {
      id: "environments",
      title: "equation, align, gather, cases, matrices",
      body: `
<h4>One numbered formula</h4>
${show(String.raw`\begin{equation}\label{eq:energy}
  E = mc^2 .
\end{equation}`, String.raw`E = mc^2 . \qquad (1)`)}
<h4>Several lines aligned: <code>align</code> / <code>align*</code></h4>
${show(String.raw`\begin{align*}
  (a+b)^2 &= (a+b)(a+b) \\
          &= a^2 + 2ab + b^2 .
\end{align*}`, String.raw`\begin{aligned} (a+b)^2 &= (a+b)(a+b) \\ &= a^2 + 2ab + b^2 . \end{aligned}`,
  `${c`&`} marks the alignment point (just before the ${c`=`}); ${c`\\`} ends a row, but <em>not</em> the last one. In ${c`align`} every row is numbered; ${c`\notag`} drops one number. Several ${c`&`} pairs give side-by-side columns.`)}
<h4>Centred lines with no alignment: <code>gather</code></h4>
${show(String.raw`\begin{gather*}
  x + y = 1 , \\
  x - y = 3 .
\end{gather*}`, String.raw`\begin{gathered} x + y = 1 , \\ x - y = 3 . \end{gathered}`)}
<h4>One number for a multi-line derivation: <code>split</code> or <code>aligned</code> inside <code>equation</code></h4>
${show(String.raw`\begin{equation}\label{eq:sum}
\begin{split}
  \sum_{k=1}^{n} k &= 1 + 2 + \dots + n \\
                   &= \frac{n(n+1)}{2} .
\end{split}
\end{equation}`, String.raw`\begin{aligned} \sum_{k=1}^{n} k &= 1 + 2 + \dots + n \\ &= \frac{n(n+1)}{2} . \end{aligned} \qquad (2)`)}
<h4>Cases</h4>
${show(String.raw`\[
  \lvert x \rvert =
  \begin{cases}
    x  & \text{if } x \ge 0 , \\
    -x & \text{if } x < 0 .
  \end{cases}
\]`, String.raw`\lvert x \rvert = \begin{cases} x & \text{if } x \ge 0 , \\ -x & \text{if } x < 0 . \end{cases}`,
  `${c`dcases`} (mathtools, already loaded) makes fractions inside cases full size.`)}
<h4>Matrices</h4>
${show(String.raw`\begin{pmatrix} a & b \\ c & d \end{pmatrix}
\begin{bmatrix} 1 & 0 \\ 0 & 1 \end{bmatrix}
\begin{vmatrix} a & b \\ c & d \end{vmatrix} = ad - bc`, String.raw`\begin{pmatrix} a & b \\ c & d \end{pmatrix} \quad \begin{bmatrix} 1 & 0 \\ 0 & 1 \end{bmatrix} \quad \begin{vmatrix} a & b \\ c & d \end{vmatrix} = ad - bc`,
  `${c`pmatrix`} ( ), ${c`bmatrix`} [ ], ${c`vmatrix`} determinant bars, ${c`Vmatrix`} norm bars. Inside a sentence use ${c`smallmatrix`}. Tab types ${c`&`}, Enter a new row (see Latex Suite features).`)}
<p><code>multline</code> is for one formula too long for a line (first part left, last part right).</p>`,
    },
    {
      id: "punctuation",
      title: "Punctuation in display maths",
      body: `
<p>A displayed formula is part of the sentence, so it takes the punctuation the sentence needs, <em>inside</em> the maths. The text after it continues in lower case when the sentence goes on.</p>
${show(String.raw`Hence
\[
  \int_0^1 x^2 \, dx = \frac{1}{3} ,
\]
where the integral is taken over $[0,1]$.`, String.raw`\int_0^1 x^2 \, dx = \frac{1}{3} ,`)}
<p>End with ${c`.`} when the sentence ends there. In ${c`align`}, put the comma or full stop at the end of each row, before ${c`\\`}.</p>`,
    },
    {
      id: "words-in-math",
      title: "Words and operator names in maths",
      body: `
${show(String.raw`f(x) = 0 \quad \text{for all } x \in A`, null, `${c`\text{…}`} keeps words upright with normal spacing; put the spaces inside the braces. (Your ${c`text`} and ${c`"`} snippets type ${c`\text{}`}.)`)}
${show(String.raw`\operatorname{Tr}(A) , \quad \sin x , \quad \log n`, null, `Named functions are upright and get operator spacing: ${c`\sin`}, ${c`\log`}, ${c`\det`}, … exist already. For others use ${c`\operatorname{Tr}`}, or once in the preamble ${c`\DeclareMathOperator{\Tr}{Tr}`} and then write ${c`\Tr(A)`}.`)}
${src`\DeclareMathOperator{\Tr}{Tr}          % in the preamble
\DeclareMathOperator*{\argmax}{arg\,max} % * = limits go underneath in display`}
<p>Not ${c`Tr(A)`} (italic letters T·r) and not ${c`\text{Tr}(A)`} (no operator spacing).</p>`,
    },
    {
      id: "spacing",
      title: "Spacing",
      body: `
<p>Spaces you type in maths are ignored; LaTeX spaces maths itself. Add space only where the meaning needs it:</p>
<table class="compare"><thead><tr><th scope="col">Command</th><th scope="col">Width</th><th scope="col">Typical use</th></tr></thead><tbody>
<tr><td>${c`\,`}</td><td>thin</td><td>before ${c`dx`}: ${tex`\int_0^1 f(x)\,dx`}; number and unit: ${tex`5\,\mathrm{cm}`}</td></tr>
<tr><td>${c`\;`}</td><td>thick</td><td>after a quantifier's comma: ${tex`\forall x \in A,\; x > 0`}</td></tr>
<tr><td>${c`\quad`}, ${c`\qquad`}</td><td>1 em, 2 em</td><td>between a formula and a condition: ${tex`x = 1 \quad \text{if } y = 0`}</td></tr>
<tr><td>${c`\!`}</td><td>negative thin</td><td>pulling things closer: ${tex`\int\!\!\int`} (${c`\int\!\!\int`}; but prefer ${c`\iint`} for that)</td></tr>
</tbody></table>
<p>Your integral snippets (${c`dint`}, ${c`\int`} Tab, ${c`oinf`}, ${c`infi`}) already type ${c`\,dx`}.</p>`,
    },
    {
      id: "delimiters",
      title: "Brackets and delimiters",
      body: `
${show(String.raw`\left( \frac{a}{b} \right)^2 \qquad \bigl( f(x) \bigr) \qquad \lvert x \rvert \qquad \lVert v \rVert \qquad \langle u, v \rangle`, null,
  `${c`\left( … \right)`} grows to fit what is inside (auto-enlarge adds it for ${c`\sum`}, ${c`\frac`}, …). When it comes out too big, size by hand with ${c`\bigl( \bigr)`}, ${c`\Bigl( \Bigr)`}, ${c`\biggl`}.`)}
<ul>
<li>Absolute value ${c`\lvert x \rvert`} and norm ${c`\lVert v \rVert`}, not ${c`|x|`} and ${c`||v||`}: the l/r versions get the right spacing (your ${c`norm`} and ${c`Norm`} snippets type them).</li>
<li>Angle brackets ${c`\langle u, v \rangle`}, never ${c`<u,v>`} (those are less-than signs). Snippets: ${c`avg`}, ${c`odp`}.</li>
<li>With mathtools, ${c`\DeclarePairedDelimiter\abs{\lvert}{\rvert}`} in the preamble gives ${c`\abs{x}`}, and ${c`\abs*{\frac{a}{b}}`} sizes automatically.</li>
<li>Braces need a backslash: ${c`\{ 1, 2, 3 \}`}.</li>
</ul>`,
    },
    {
      id: "dots",
      title: "Dots",
      body: `
${show(String.raw`a_1, \dots, a_n \qquad a_1 + \dots + a_n \qquad a_1 + \cdots + a_n \qquad \vdots \quad \ddots`, null,
  `Type ${c`\dots`} (snippet ${c`...`}) and amsmath picks low dots between commas and centred dots between operators, from the symbol that follows. To force it: ${c`\ldots`} low, ${c`\cdots`} centred; ${c`\dotsc`} (commas), ${c`\dotsb`} (binary operators). ${c`\vdots`} and ${c`\ddots`} are for matrices. Never three full stops.`)}`,
    },
    {
      id: "bold",
      title: "Bold vectors and matrices",
      body: `
${show(String.raw`\mathbf{v} = (v_1, v_2) , \quad \mathbf{A}\mathbf{x} = \mathbf{b} , \quad \boldsymbol{\mu} , \quad \vec{F}`, null,
  `${c`\mathbf{v}`} for Latin letters (upright bold). For Greek use ${c`\boldsymbol{\mu}`} (or ${c`\bm{\mu}`} with the bm package): ${c`\mathbf`} leaves lower-case Greek unchanged and makes capitals upright bold, not italic. Snippets: letter + ${c`,.`} or ${c`.,`} → ${c`\mathbf{}`}, ${c`\mu,.`} → ${c`\boldsymbol{\mu}`}. Arrows (${c`\vec`}) are the physics habit. Choose one convention per document.`)}`,
    },
    {
      id: "sets-logic",
      title: "Sets and logic",
      body: `
${show(String.raw`\{\, x \in \mathbb{R} : x > 0 \,\} \qquad A \subseteq B \qquad A \setminus B \qquad f\colon A \to B,\; x \mapsto x^2`, null, null)}
<ul>
<li>Set-builder: ${c`\{\, x \in \mathbb{R} : x > 0 \,\}`} (or ${c`\mid`} instead of ${c`:`}); the thin spaces are optional polish.</li>
<li>${c`\subseteq`} (snippet ${c`sub`}) vs ${c`\subset`} (${c`psub`}): authors differ on whether ${c`\subset`} means proper; say which you use, or write ${c`\subsetneq`}.</li>
<li>Difference ${c`\setminus`} (snippets ${c`diff`}, ${c`\\\\\\`}), not ${c`-`} or ${c`\backslash`}. Empty set ${c`\emptyset`} or ${c`\varnothing`} (${c`eset`}).</li>
<li>Functions: ${c`f\colon A \to B`}: ${c`\colon`} is punctuation; plain ${c`:`} is a relation with wide spacing. Elements: ${c`x \mapsto x^2`}.</li>
<li>${c`\implies`} and ${c`\iff`} (wide, for steps of an argument) vs ${c`\Rightarrow`} (a narrow arrow symbol). In prose, the words “implies”, “if and only if” often read better.</li>
<li>${c`\mathbb{N}, \mathbb{Z}, \mathbb{Q}, \mathbb{R}, \mathbb{C}`} need amssymb (in the template).</li>
</ul>`,
    },
    {
      id: "theorems",
      title: "Theorems and proofs",
      body: `
<p>The template defines these (amsthm). theorem, lemma, proposition, corollary share one counter, numbered within the section (Theorem 2.3, Lemma 2.4); definition and example are upright; remark is unnumbered.</p>
${src`\begin{theorem}[Cauchy--Schwarz]\label{thm:cs}
  For all $u, v$ in an inner product space,
  $\lvert \langle u, v \rangle \rvert \le \lVert u \rVert \, \lVert v \rVert$.
\end{theorem}

\begin{proof}
  If $v = 0$ there is nothing to show. Otherwise \dots
  \[
    0 \le \lVert u - t v \rVert^2 . \qedhere
  \]
\end{proof}`}
<ul>
<li>${c`proof`} adds the end mark itself: amsthm's default is the open box □. When the proof ends with a display, put ${c`\qedhere`} inside it, or the mark lands on a line of its own. (For a filled ∎ instead: ${c`\renewcommand{\qedsymbol}{$\blacksquare$}`} in the preamble.)</li>
<li>Your ${c`qed`} snippet types ${c`\ \blacksquare`} (∎): fine in notes, but inside ${c`proof`} it adds a second mark next to the □.</li>
<li>${c`\begin{proof}[Proof of Theorem~\ref{thm:cs}]`} changes the heading.</li>
</ul>`,
    },
    {
      id: "labels",
      title: "Labels and references",
      body: `
<table class="compare"><thead><tr><th scope="col">Prefix</th><th scope="col">For</th><th scope="col">Refer with</th></tr></thead><tbody>
<tr><td>${c`eq:`}</td><td>equations</td><td>${c`\eqref{eq:energy}`} → (1), brackets included</td></tr>
<tr><td>${c`sec:`}</td><td>sections</td><td>${c`Section~\ref{sec:compact}`}</td></tr>
<tr><td>${c`thm:`} ${c`lem:`} ${c`prop:`} ${c`cor:`} ${c`def:`}</td><td>theorems, lemmas, …</td><td>${c`Theorem~\ref{thm:cs}`}</td></tr>
<tr><td>${c`fig:`} ${c`tab:`}</td><td>figures, tables</td><td>${c`Figure~\ref{fig:plot}`}</td></tr>
</tbody></table>
<ul>
<li>${c`\label`} goes right after what it names: after ${c`\section{…}`}, inside ${c`equation`}, after the ${c`\caption`} of a figure, in the row of an ${c`align`}.</li>
<li>Unnumbered things (${c`\[ \]`}, ${c`equation*`}, ${c`align*`}) cannot be referenced.</li>
<li>${c`~`} before ${c`\ref`} keeps “Theorem” and its number on one line.</li>
<li>Ctrl-Space inside ${c`\ref{`} / ${c`\eqref{`} lists your labels (texlab).</li>
<li>“??” in the PDF means the label doesn't exist (a typo, or it's on an unnumbered equation): tectonic reruns LaTeX by itself, so a single compile resolves every reference.</li>
</ul>`,
    },
    {
      id: "typography",
      title: "Text typography",
      body: `
<table class="compare"><thead><tr><th scope="col">Write</th><th scope="col">Get</th><th scope="col">Use</th></tr></thead><tbody>
<tr><td>${c`\`\`quoted''`}</td><td>“quoted”</td><td>double quotes: two backticks, two apostrophes (or type “ ” directly). Never ${c`"`}.</td></tr>
<tr><td>${c`\`single'`}</td><td>‘single’</td><td>single quotes</td></tr>
<tr><td>${c`-`}</td><td>-</td><td>hyphen: well-known, non-negative</td></tr>
<tr><td>${c`--`}</td><td>–</td><td>en dash: ranges 3--5, pages 10--12, Cauchy--Schwarz</td></tr>
<tr><td>${c`---`}</td><td>—</td><td>em dash, for a break in a sentence</td></tr>
<tr><td>${c`$-1$`}</td><td>${tex`-1`}</td><td>a minus sign is maths, not a hyphen</td></tr>
<tr><td>${c`Figure~\ref{…}`}</td><td>Figure 3</td><td>${c`~`} is a space that never breaks: before refs, ${c`Dr.~Smith`}, ${c`$n$~points`}</td></tr>
<tr><td>${c`\emph{new term}`}</td><td><em>new term</em></td><td>emphasis and defined terms; not bold, not ${c`\textit`}</td></tr>
<tr><td>${c`e.g.\ this`}</td><td>e.g. this</td><td>${c`\ `} after an abbreviation's full stop, so it isn't spaced like a sentence end</td></tr>
<tr><td>${c`\dots`}</td><td>…</td><td>ellipsis in text too</td></tr>
</tbody></table>`,
    },
    {
      id: "lists",
      title: "Lists",
      body: `
${src`\begin{itemize}          % bullets
  \item First point.
  \item Second point.
\end{itemize}

\begin{enumerate}        % 1. 2. 3.
  \item \label{it:first} Step one.
\end{enumerate}

\begin{description}
  \item[Compact] Every open cover has a finite subcover.
\end{description}`}
<p>Other labels, e.g. (a), (b): ${c`\usepackage{enumitem}`} and ${c`\begin{enumerate}[label=(\alph*)]`}.</p>`,
    },
    {
      id: "floats",
      title: "Figures and tables",
      body: `
${src`\usepackage{graphicx, booktabs}   % preamble

\begin{figure}[htbp]
  \centering
  \includegraphics[width=0.7\linewidth]{plot.pdf}
  \caption{Error against step size.}\label{fig:error}
\end{figure}

\begin{table}[htbp]
  \centering
  \caption{Timings.}\label{tab:timing}  % tables: caption above
  \begin{tabular}{lrr}
    \toprule
    Method & Steps & Time (s) \\
    \midrule
    Euler  & 1000  & 0.12 \\
    RK4    & 100   & 0.05 \\
    \bottomrule
  \end{tabular}
\end{table}`}
<ul>
<li>${c`\label`} after ${c`\caption`}, or the reference points at the section instead.</li>
<li>booktabs: ${c`\toprule`}, ${c`\midrule`}, ${c`\bottomrule`}, no vertical lines, no ${c`\hline`}.</li>
<li>Figures float to a good place (${c`[htbp]`}); refer to them by number, not “below”.</li>
</ul>`,
    },
    {
      id: "packages",
      title: "Common packages",
      body: `
<table class="compare"><thead><tr><th scope="col">Package</th><th scope="col">Gives</th><th scope="col">In the template?</th></tr></thead><tbody>
<tr><td>${c`amsmath`}</td><td>align, gather, cases, ${c`\text`}, ${c`\operatorname`}, ${c`\eqref`}, ${c`\dots`}</td><td>yes</td></tr>
<tr><td>${c`amssymb`}</td><td>${c`\mathbb`}, ${c`\varnothing`}, ${c`\therefore`}, ${c`\blacksquare`}, …</td><td>yes</td></tr>
<tr><td>${c`amsthm`}</td><td>theorem environments, proof, ${c`\qedhere`}</td><td>yes</td></tr>
<tr><td>${c`mathtools`}</td><td>fixes and extras for amsmath: ${c`dcases`}, ${c`\coloneqq`}, ${c`\DeclarePairedDelimiter`}</td><td>yes</td></tr>
<tr><td>${c`geometry`}</td><td>page margins</td><td>yes (1 in)</td></tr>
<tr><td>${c`cancel`}</td><td>${c`\cancel`}, ${c`\cancelto`}: needed by your visual snippets C and K</td><td class="bad">no: add it if you use them</td></tr>
<tr><td>${c`physics`} or ${c`braket`}</td><td>${c`\bra`}, ${c`\ket`}, ${c`\braket`}: needed by ${c`bra`}, ${c`ket`}, ${c`brk`}, ${c`outer`}</td><td class="bad">no</td></tr>
<tr><td>${c`mhchem`}</td><td>${c`\ce`}, ${c`\pu`} (${c`\usepackage[version=4]{mhchem}`}): needed by ${c`cee`}, ${c`pu`}</td><td class="bad">no</td></tr>
<tr><td>${c`graphicx`}, ${c`booktabs`}</td><td>images, good tables</td><td>no</td></tr>
<tr><td>${c`enumitem`}</td><td>list labels and spacing</td><td>no</td></tr>
<tr><td>${c`siunitx`}</td><td>${c`\qty{5}{\centi\metre}`}, aligned number columns</td><td>no</td></tr>
<tr><td>${c`hyperref`}, then ${c`cleveref`}</td><td>clickable refs; ${c`\Cref{thm:cs}`} writes “Theorem 1.2” itself (${c`\cref`}: lower case, “theorem 1.2”, for mid-sentence). Load last.</td><td>no</td></tr>
<tr><td>${c`microtype`}</td><td>subtly better spacing and line breaks</td><td>no</td></tr>
</tbody></table>`,
    },
    {
      id: "mistakes",
      title: "Common mistakes",
      body: `
<ol class="mistakes">
<li>${c`$$ … $$`} in a .tex file (pasted from Markdown) → ${c`\[ … \]`}. (${c`dm`} already types ${c`\[ \]`} in .tex.)</li>
<li>${c`eqnarray`} → ${c`align`}.</li>
<li>${c`\\`} after the last row of ${c`align`} / ${c`cases`} / a matrix → an empty extra row. Leave it off.</li>
<li>A blank line before, inside or after display maths → remove it.</li>
<li>${c`sin x`}, ${c`Tr A`} → ${c`\sin x`}, ${c`\operatorname{Tr} A`}.</li>
<li>${c`f: A \to B`} → ${c`f\colon A \to B`}; ${c`<u,v>`} → ${c`\langle u, v \rangle`}; ${c`|x|`} → ${c`\lvert x \rvert`}.</li>
<li>${c`x^10`} gives ${tex`x^10`} → ${c`x^{10}`} (braces around anything longer than one character; snippet ${c`rd`}).</li>
<li>Words in maths without ${c`\text`}: ${c`x = 1 if y`} → ${c`x = 1 \text{ if } y`}.</li>
<li>${c`...`} → ${c`\dots`}; ${c`"quotes"`} → ${c`\`\`quotes''`}.</li>
<li>${c`\label`} before ${c`\caption`}, or on an unnumbered equation.</li>
<li>${c`\ref`} for equations → ${c`\eqref`}; missing ${c`~`} before ${c`\ref`}.</li>
<li>${c`\left( \right)`} around everything → only where the contents are tall.</li>
<li>Old font switches ${c`{\bf x}`}, ${c`{\it x}`}, ${c`\rm`} → ${c`\textbf{x}`}, ${c`\emph{x}`}, ${c`\mathbf{x}`}, ${c`\mathrm{x}`}.</li>
<li>A ∎ typed by hand inside ${c`proof`} → let ${c`proof`} add it; use ${c`\qedhere`} after a display.</li>
<li>${c`\cancel`}, ${c`\ket`}, ${c`\ce`} without their package → see Common packages.</li>
</ol>`,
    },
  ];
}
