import { observable } from "mobx";

export interface VideoPlayer {
	playFrom(ms: number): void;
}

export class Model {
	@observable.ref videoPlayer: VideoPlayer | undefined;
	@observable.ref matchings: Matching[] | undefined = undefined;
	@observable currentVideoHash: string | undefined = undefined;

	/** does not end with slash */
	public readonly serverUrl: string;

	public readonly pdfHash: string;

	public get pdfUrl(): string {
		return `${this.serverUrl}/files/${this.pdfHash}`;
	}

	public get videoUrl(): string | undefined {
		if (!this.currentVideoHash) {
			return undefined;
		}
		return `${this.serverUrl}/files/${this.currentVideoHash}`;
	}

	public async fetchMatchings(): Promise<Matching[]> {
		const result = await fetch(
			`${this.serverUrl}/pdf-matchings/${this.pdfHash}`
		);
		const json = await result.json();
		interface RustMatching {
			video_offset_ms: number;
			pdf_hash: string;
			video_hash: string;
			page_idx: number;
			duration_ms: number;
		}

		const matchings = json as RustMatching[];
		return matchings.map((m) => ({
			videoOffsetMs: m.video_offset_ms,
			pdfHash: m.pdf_hash,
			videoHash: m.video_hash,
			pageIdx: m.page_idx,
			durationMs: m.duration_ms,
		}));
	}

	constructor() {
		const urlParams = new URLSearchParams(window.location.search);
		let serverUrl = urlParams.get("server-url") || "";
		if (serverUrl.endsWith("/")) {
			serverUrl = serverUrl.substr(0, serverUrl.length - 1);
		}
		this.serverUrl = serverUrl;

		const pdfHash = urlParams.get("pdf-hash");
		if (!pdfHash) {
			alert("No pdf hash!");
			throw new Error();
		}
		this.pdfHash = pdfHash;

		this.fetchMatchings().then((m) => (this.matchings = m));
	}
}

export interface Matching {
	videoOffsetMs: number;
	pdfHash: string;
	videoHash: string;
	pageIdx: number;
	durationMs: number;
}
