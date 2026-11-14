# Licensing and reuse

Decision: 14 November 2026. Roller Hopper, including its development tools, is offered under
**GPL-3.0-or-later**: GNU General Public License version 3 or, at your option, any later version.
The full version 3 text is in [LICENSE](../LICENSE); Cargo package metadata uses the same SPDX ID.

## Why this licence

The goal is compatibility with the source candidates in the [reuse review](github-reuse-review.md),
including potential GPL implementation reuse. MIT or Apache-2.0 alone would not cover a combined
derivative containing GPL code. GPLv3 accommodates the reviewed MIT, Apache-2.0 and BSD-3-Clause
material and compatible LGPL/GPL material, subject to each component's actual terms.
See the [GNU licence list](https://www.gnu.org/licenses/license-list.html),
[Apache's compatibility explanation](https://www.apache.org/licenses/GPL-compatibility.html)
and [Slint's GPL option](https://slint.dev/community).

This choice prioritises incorporating the reviewed code into an open-source application.
Recipients distributing a combined GPL work must satisfy GPL requirements, including supplying
corresponding source through a compliant method. It is not a permissive licence for proprietary
combined derivatives. Private use does not itself require publishing changes.

Slint's GPLv3 option is the intended route for the combined application. Check the licence files
of the pinned release and its selected backends before packaging. This decision is not a completed
audit of dependencies, icons, model weights or other distributable assets.

## Existing rights and third-party material

The previous project MIT notice is preserved in [LICENSE-MIT](../LICENSE-MIT) for material already
released under that grant. Those permissions are not revoked. That historical notice does not
offer future GPL-only contributions or third-party GPL code under MIT.

Upstream components retain their copyright, licence and notice requirements. The project licence
does not relicense imported source, permit copying unlicensed files, or clear mixed-licence assets.
The private reference bundles remain reference material; this change incorporates no candidate code.

The reuse review records the earlier MIT baseline. Its warnings against presenting GPL ports as
MIT-only remain valid, but GPL reuse can now be considered for this GPL application. Its preference
for small native implementations and its technical assessments still apply. LGPL translations and
ports retain upstream obligations; using an external API is not an automatic licence exemption.

Before the first source port or new dependency, record its exact upstream commit/path or release,
chosen licence, copyright and retained notice paths, local modifications and dependency/asset review
in a third-party attribution record committed with that adoption. Include required licences,
NOTICE content, modification notices and corresponding source in the distribution as applicable.
Do not add attribution entries suggesting that unadopted reference projects are shipped.
