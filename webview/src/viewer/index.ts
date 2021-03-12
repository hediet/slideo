import "./style.scss";
import { loadViewer, PDFPageView } from "@hediet/pdfjs-viewer";
import { PdfMatching, pdfViewerContract } from "./contract";
import { ConsoleRpcLogger, Contract } from "@hediet/json-rpc";
import { connectIFrameToParent } from "@hediet/json-rpc-browser";

let pdfMatchings: PdfMatching[] = [];

const { client } = Contract.registerServerToStream(
	pdfViewerContract,
	connectIFrameToParent(),
	{ sendExceptionDetails: true, logger: new ConsoleRpcLogger() },
	{
		openPdf: async ({ pdfUrl, matchings }) => {
			pdfMatchings = matchings;
			loadViewer({
				pdfUrl,
			});
			return {};
		},
	}
);

function msToTime(s: number): string {
	function pad(n: number): string {
		return ("00" + n).slice(-2);
	}

	const ms = s % 1000;
	s = (s - ms) / 1000;
	const secs = s % 60;
	s = (s - secs) / 60;
	const mins = s; // % 60;
	//const hrs = (s - mins) / 60;

	return pad(mins) + ":" + pad(secs);
}

const old = PDFPageView.prototype.draw;
PDFPageView.prototype.draw = function (...args) {
	const result = old.apply(this, args);

	const div = this.div;

	let m = pdfMatchings.filter((m) => m.pageIdx === this.id - 1);
	if (m.length > 0) {
		const playButtons = document.createElement("div");
		div.appendChild(playButtons);

		playButtons.style.position = "absolute";
		playButtons.style.top = "0";
		playButtons.style.right = "0";
		playButtons.style.margin = "10px";

		playButtons.innerHTML = `
    <div style="display: flex; align-content: center">
		<div class="playButton" style="display: flex; align-items: center;">
			<svg data-icon="play" width="22" height="22" viewBox="0 0 20 20"><desc>play</desc><path d="M16 10c0-.36-.2-.67-.49-.84l.01-.01-10-6-.01.01A.991.991 0 005 3c-.55 0-1 .45-1 1v12c0 .55.45 1 1 1 .19 0 .36-.07.51-.16l.01.01 10-6-.01-.01c.29-.17.49-.48.49-.84z" fill-rule="evenodd"></path></svg>
			<div style="width: 2px"></div>
			<span>${msToTime(m[0].durationMs)} min</span>
		</div>
	<div>
    `;
		playButtons.onclick = () => {
			client.playVideo({
				offsetMs: m[0].videoOffsetMs,
				videoHash: m[0].videoHash,
			});
		};
	}

	return result;
};

client.initialized({});
