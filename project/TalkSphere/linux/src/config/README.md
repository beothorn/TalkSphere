# Config

This module is responsible for editing user configuration stored in the app home `config` file.

The file currently uses one `key=value` entry per line. Scalar configuration, such as `availability`, is replaced with `config set`. List configuration, such as `reachableAt`, uses repeated lines and is changed with `config add` and `config remove`.
