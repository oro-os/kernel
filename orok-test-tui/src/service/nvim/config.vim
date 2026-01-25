" We prevent closing vim as it'll exit the entire app.
" This isn't exhaustive. It's hard to be exhaustive.
cabbrev q <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'q')<CR>
cabbrev wq <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'wq')<CR>
cabbrev cq <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'cq')<CR>
cabbrev qa <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'qa')<CR>
cabbrev qall <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'qall')<CR>
cabbrev wqa <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'wqa')<CR>
cabbrev wqall <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Use Ctrl-D to exit Oro Kernel TUI"' : 'wqall')<CR>
cabbrev vsplit <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Splitting will break Oro Kernel TUI"' : 'vsplit')<CR>
cabbrev split <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Splitting will break Oro Kernel TUI"' : 'split')<CR>
cabbrev new <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Splitting will break Oro Kernel TUI"' : 'new')<CR>
cabbrev vnew <c-r>=(getcmdtype()==':' && getcmdpos()==1 ? 'echo "Splitting will break Oro Kernel TUI"' : 'vnew')<CR>

" Don't show splash
set shortmess+=I

" Set up tabs
set tabstop=4
set shiftwidth=4
set softtabstop=4
set noexpandtab

" Show whitespace
set list
set listchars=tab:→\ ,trail:·,nbsp:␣,extends:»,precedes:«

" Show line numbers
set number

" Use specific theme
colors wildcharm

" Always show the sign columne
set signcolumn=yes:1
