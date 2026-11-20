select md5(name) as a, sha1(nick) as b, sha256(data) as c, sha384(name) as d, sha512(name) as e, blake3(nick) as f, crypto_md5(:text) as g from people
