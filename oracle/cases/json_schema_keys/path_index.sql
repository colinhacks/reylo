select doc #> '{tags,0}' as first_tag, doc #> '{0}' as zero, doc -> '0' as zero_key, l #> '{0,a}' as first_a, l -> 'a' as a from docs, lists
