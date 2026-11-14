create macro tm(n) as table select * from range(n); comment on macro table tm is 'rows';
