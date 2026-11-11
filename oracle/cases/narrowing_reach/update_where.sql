update orders set total = 1 where note = :n returning note, total
