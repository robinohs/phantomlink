<p align="center">
  <img src="./docs/images/phantomlink.png" alt="phantomlink Logo" width="500px">
</p>

<p align="center"><code>phantomlink</code> looks like a multi-hop Internet path but emulates a virtual end-to-end link</p>

<p align="center">
    <a href="https://www.rustup.rs"><img alt="Minimum Stable Rust Version 1.7.4" src="https://img.shields.io/badge/Rust-1.74.1%2B-orange.svg"></a>
    <a href="https://depend.cs.uni-saarland.de/"><img alt="Dependable Systems and Software" src="https://img.shields.io/badge/Dependable%20Systems%20and%20Software-8A2BE2"></a>
</p>

<code>phantomlink</code> is a tool for studying the impact of a dynamic network environment on the performance of upperlying protocols and applications that make use of them. LOREM IPSUM

## 🔥 Features

- **connect** two Linux network namespaces over a **dynamic virtual link**
- specify **bottleneck data rate**, **link delay**, and **packet reordering**
- set the virtual link behavior with a **simple input file format**
- **support** for **all protocols** using ethernet frames ➡️ just route the traffic to <code>phantomlink</code>

## 🔍 Structure of ```phantomlink```
### Toolchain Architecture

Lorem Ipsum

### Core Binary Architecture

Lorem Ipsum

## 🚀 Use ```phantomlink```

> [!WARNING]
> Phantomlink currently only runs on Linux or the Windows Subsystem for Linux (WSL)

Phantomlink is available for download as a binary here on GitHub.
At the moment it is still required to setup the namespace environment manually.
We provide a bunch of scripts, which handle the most common operations.
To get started you have to do the following:

  1.  Download the phantomlink binary, make it executable and move it to `/bin` respc. `/usr/bin`
  2.  Create a new folder and download the scripts folder.
  3.  Double check and execute the `setup.sh` script.
  4.  Double check and run the `run.sh` script.

## � FAQ

### I need help!

Don't hesitate to file an issue or contact one of the authors!

### How can I help?

Please have a look at the issues or open one if you feel that something is needed.

Any contributions are very welcome!

## 🏛 License

`Phantomlink` is licensed under [MIT](https://github.com/robinohs/phantomlink/blob/main/LICENSE).

## 🙏 Acknowledgement

This project has received funding from the European Union’s Horizon 2020 research and innovation programme under the Marie Skłodowska-Curie grant agreement No [101008233](https://doi.org/10.3030/101008233) (MISSION).

## 🤼 Contributing

We look forward to any kind of contributions!

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be MIT licensed as above, without any additional terms or conditions.
