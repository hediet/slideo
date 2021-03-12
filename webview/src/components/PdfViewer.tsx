import { ConsoleRpcLogger, Contract, RpcStreamLogger } from "@hediet/json-rpc";
import { connectToIFrame } from "@hediet/json-rpc-browser";
import { autorun, observable } from "mobx";
import { disposeOnUnmount } from "mobx-react";
import React = require("react");
import { Matching } from "../model";
import { pdfViewerContract } from "../viewer/contract";

export class PdfViewer extends React.Component<{
	pdfUrl: string;
	playVideo: (offsetMs: number, videoHash: string) => void;
	matchings: Matching[];
}> {
	private iframe: HTMLIFrameElement | null = null;
	@observable.ref
	private pdfFrame!: typeof pdfViewerContract["TServerInterface"];

	/*
	@disposeOnUnmount
	private readonly _updateMatchings = autorun(() => {
		if (this.pdfFrame) {
			const matchings = this.props.matchingsSource();
			this.pdfFrame.updateMatchings({ matchings });
		}
	});
	*/

	setupIFrame(iframe: HTMLIFrameElement | null) {
		if (!iframe) {
			return;
		}

		const { server } = Contract.getServerFromStream(
			pdfViewerContract,
			connectToIFrame(iframe),
			{ sendExceptionDetails: true, logger: new ConsoleRpcLogger() },
			{
				playVideo: ({ offsetMs, videoHash }) => {
					this.props.playVideo(offsetMs, videoHash);
				},
				initialized: ({}, { counterpart }) => {
					counterpart.openPdf({
						matchings: this.props.matchings,
						pdfUrl: this.props.pdfUrl,
					});
				},
			}
		);

		this.pdfFrame = server;
	}

	render() {
		return (
			<iframe
				style={{ height: "100%", width: "100%", border: 0 }}
				ref={(ref) => this.setupIFrame(ref)}
				src="/viewer.html"
			/>
		);
	}
}
