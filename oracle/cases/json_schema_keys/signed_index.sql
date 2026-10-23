select m #> '{tags,-1}' as last, m #> '{tags,+0}' as first, m #> '{tags," 1"}' as second, m #> '{tags,a}' as a from mixed
