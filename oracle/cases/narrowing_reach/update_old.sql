update orders set note = null where note = :n returning old.note as before, new.note as after
