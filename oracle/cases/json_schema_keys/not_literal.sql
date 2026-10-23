select doc -> :key::text as v, doc ->> ('na' || 'me') as w from docs
