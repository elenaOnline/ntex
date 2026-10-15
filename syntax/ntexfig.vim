" Ntex Figures shorthand (.fig): studio/figures/FIGURES.md "The shorthand".
" lua/config/figures.lua sets the filetype; ftplugin/ntexfig.lua the options.
if exists("b:current_syntax")
  finish
endif

syn case match
syn sync minlines=50

" LaTeX inside tokens and labels: \mathbb, \ker, \varphi
syn match ntexfigCommand /\\[A-Za-z]\+\*\?/ contained
syn match ntexfigCommand /\\[A-Za-z]\+\*\?/

" Numbers in positions and sizes: at 1,0  at (3.2,-1)  spacing 2.8,2  bend=40
syn match ntexfigNumber /\v(^|[ \t,(=])@<=-?\d+(\.\d+)?>/

" Statements (at the start of a line or after ;)
syn match ntexfigKeyword /\v((^|;)\s*)@<=<(blob|ngon|cayley|text|scale|spacing|size)>/
syn match ntexfigWord /\v(^|\s)@<=(at|in)(\s|$)@=/

" what blob / ngon name
syn match ntexfigName /\v((^|;)\s*(blob|ngon)\s+)@<=[^ \t"{};]+/
" cayley D4 / Z6 / Z2 x Z3, and presentations <a, b | a^4, b^2>
syn match ntexfigGroup /\v((^|;)\s*cayley\s+)@<=[A-Za-z]+\d*(\s+x\s+[A-Za-z]+\d+)?/
syn match ntexfigGenerators /\v\<[^-=<>.|][^<>]*\>/ contains=ntexfigCommand

" Options: bare words, and key=value
syn match ntexfigOption /\v(\s)@<=(bend-left|bend-right|near-start|near-end|bend|loop|swap|iso|dashed|dotted|thick|shaded|legend|axes|axis|labels|rotation)(\s|;|\=|$)@=/
syn match ntexfigOption /\v(\s)@<=(color|shape|size|label|labels|layout|turn|flip|radius|bend|rotation)\=@=/ nextgroup=ntexfigEquals
syn match ntexfigEquals /=/ contained nextgroup=ntexfigValue
syn match ntexfigValue /[^ \t;{}"]\+/ contained contains=ntexfigNumber,ntexfigPalette
syn keyword ntexfigPalette blue violet teal maroon green ochre grey ink contained
syn keyword ntexfigPalette round tall wide bean above below left right inside circle prism grid none contained

" Arrows: -> -f-> ->> >-> hook-> => --> ..> <-f- <-> = |-> ↦ (labels between the dashes)
syn match ntexfigArrowLabel /\v([-=.])@<=(hook)@![^-=.<>| \t"]+([-=.])@=/ contained contains=ntexfigCommand
syn match ntexfigArrow /\v(^|\s)@<=(\<[-=.]|\>-|\|-|[-=.]|hook-)\S{-}([>]|-)(\s|;|$)@=/ contains=ntexfigArrowLabel
syn match ntexfigArrow /\v(^|\s)@<=\=(\s|$)@=/
syn match ntexfigArrow /[↦→⇒]/
" a quoted label inside an arrow: -"g \circ f"->
syn match ntexfigArrow /\v(^|\s)@<=[<>]?[-=.]{1,2}("[^"]*")@=/
syn match ntexfigArrow /\v("[^"]*")@<=[-=.]{1,2}(hook-)?\>{1,2}/
syn match ntexfigArrow /\v("[^"]*")@<=[-=.]{1,2}(\s|$)@=/

" Grid blocks: ASCII art between grid and end
syn region ntexfigGrid matchgroup=ntexfigGridKeyword start=/^\s*grid\s*$/ end=/^\s*end\s*$/ keepend
      \ contains=ntexfigComment,ntexfigString,ntexfigCommand,ntexfigArrow,ntexfigGridArrow,ntexfigGridVert
" - - > (dashed) inside grids
syn match ntexfigGridArrow /\v(-\s)+\>/ contained
" | ‖ || : columns, v and ^ heads, \ and / diagonals; the label straight after
syn match ntexfigGridVert /\v(^|\s)@<=(\|\||[|‖:])/ contained nextgroup=ntexfigGridLabel
syn match ntexfigGridVert /\v(^|\s)@<=[v^](\s|$)@=/ contained
syn match ntexfigGridVert /\v(^|\s)@<=[\\/]/ contained nextgroup=ntexfigGridLabel
syn match ntexfigGridLabel /[^ \t]\+/ contained contains=ntexfigCommand

" Quoted labels, braces, separators, comments (last: they win)
syn region ntexfigString start=/"/ skip=/\\"/ end=/"/ oneline contains=ntexfigCommand
syn match ntexfigDelimiter /[{};,()]/
syn match ntexfigComment /\v(\\)@<!\%.*$/ contains=ntexfigTodo,@Spell
syn keyword ntexfigTodo TODO FIXME XXX NOTE contained

hi def link ntexfigComment Comment
hi def link ntexfigTodo Todo
hi def link ntexfigString String
hi def link ntexfigKeyword Statement
hi def link ntexfigGridKeyword Statement
hi def link ntexfigWord Keyword
hi def link ntexfigName Identifier
hi def link ntexfigGroup Type
hi def link ntexfigGenerators Special
hi def link ntexfigOption Type
hi def link ntexfigEquals Operator
hi def link ntexfigValue Constant
hi def link ntexfigPalette Constant
hi def link ntexfigNumber Number
hi def link ntexfigArrow Operator
hi def link ntexfigArrowLabel String
hi def link ntexfigGridArrow Operator
hi def link ntexfigGridVert Operator
hi def link ntexfigGridLabel String
hi def link ntexfigCommand Special
hi def link ntexfigDelimiter Delimiter

let b:current_syntax = "ntexfig"
