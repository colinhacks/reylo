update vkey set slug = :s where email = lower(trim(:e)) returning id
