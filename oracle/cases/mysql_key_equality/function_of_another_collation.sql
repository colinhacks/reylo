select b.latin from texts a, texts b where a.ci = :x and b.latin = lower(a.ci)
