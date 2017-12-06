error_chain! {
    foreign_links {
        Io(::std::io::Error);
    }
    links {
        LibreZip(::librezip::Error, ::librezip::ErrorKind);
    }
}