select length(name) as l, md5(name) as m, length(upper(name)) as u, abs(id) as a, length(name || 'x') as x, abs(length(name)) as n from c
