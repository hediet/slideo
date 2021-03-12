import {
	contract,
	unverifiedNotification,
	unverifiedRequest,
} from "@hediet/json-rpc";

export interface PdfMatching {
	videoOffsetMs: number;
	videoHash: string;
	pageIdx: number;
	durationMs: number;
}

export const pdfViewerContract = contract({
	name: "pdf-viewer",
	server: {
		openPdf: unverifiedRequest<
			{
				pdfUrl: string;
				matchings: PdfMatching[];
			},
			{}
		>(),
	},
	client: {
		playVideo: unverifiedNotification<{
			offsetMs: number;
			videoHash: string;
		}>(),
		initialized: unverifiedNotification(),
	},
});
