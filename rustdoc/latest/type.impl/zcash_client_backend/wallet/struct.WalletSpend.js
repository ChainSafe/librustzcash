(function() {
    var type_impls = Object.fromEntries([["zcash_client_backend",[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-WalletSpend%3CNf,+AccountId%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/zcash_client_backend/wallet.rs.html#309-332\">Source</a><a href=\"#impl-WalletSpend%3CNf,+AccountId%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Nf, AccountId&gt; <a class=\"struct\" href=\"zcash_client_backend/wallet/struct.WalletSpend.html\" title=\"struct zcash_client_backend::wallet::WalletSpend\">WalletSpend</a>&lt;Nf, AccountId&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.from_parts\" class=\"method\"><a class=\"src rightside\" href=\"src/zcash_client_backend/wallet.rs.html#311-317\">Source</a><h4 class=\"code-header\">pub fn <a href=\"zcash_client_backend/wallet/struct.WalletSpend.html#tymethod.from_parts\" class=\"fn\">from_parts</a>(index: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.usize.html\">usize</a>, nf: Nf, account_id: AccountId) -&gt; Self</h4></section></summary><div class=\"docblock\"><p>Constructs a <code>WalletSpend</code> from its constituent parts.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.index\" class=\"method\"><a class=\"src rightside\" href=\"src/zcash_client_backend/wallet.rs.html#321-323\">Source</a><h4 class=\"code-header\">pub fn <a href=\"zcash_client_backend/wallet/struct.WalletSpend.html#tymethod.index\" class=\"fn\">index</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>Returns the index of the Sapling spend or Orchard action within the transaction that\ncreated this spend.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.nf\" class=\"method\"><a class=\"src rightside\" href=\"src/zcash_client_backend/wallet.rs.html#325-327\">Source</a><h4 class=\"code-header\">pub fn <a href=\"zcash_client_backend/wallet/struct.WalletSpend.html#tymethod.nf\" class=\"fn\">nf</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.reference.html\">&amp;Nf</a></h4></section></summary><div class=\"docblock\"><p>Returns the nullifier of the spent note.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.account_id\" class=\"method\"><a class=\"src rightside\" href=\"src/zcash_client_backend/wallet.rs.html#329-331\">Source</a><h4 class=\"code-header\">pub fn <a href=\"zcash_client_backend/wallet/struct.WalletSpend.html#tymethod.account_id\" class=\"fn\">account_id</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.reference.html\">&amp;AccountId</a></h4></section></summary><div class=\"docblock\"><p>Returns the identifier to the account_id to which the note belonged.</p>\n</div></details></div></details>",0,"zcash_client_backend::wallet::WalletSaplingSpend"]]]]);
    if (window.register_type_impls) {
        window.register_type_impls(type_impls);
    } else {
        window.pending_type_impls = type_impls;
    }
})()
//{"start":55,"fragment_lengths":[3103]}