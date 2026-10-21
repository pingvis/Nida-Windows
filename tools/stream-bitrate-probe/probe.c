/* Local feasibility probe, not an application encoder. */
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/opt.h>
#include <libavutil/time.h>

static void checked(int result, const char *operation) {
    if (result < 0) {
        char message[AV_ERROR_MAX_STRING_SIZE];
        av_strerror(result, message, sizeof(message));
        fprintf(stderr, "%s: %s\n", operation, message);
        exit(1);
    }
}

static int drain(AVCodecContext *context, AVPacket *packet, AVFormatContext *out, FILE *csv) {
    int count = 0, result;
    while ((result = avcodec_receive_packet(context, packet)) >= 0) {
        fprintf(csv, "%" PRId64 ",%" PRId64 ",%d,%d\n", packet->pts, packet->dts,
                packet->size, !!(packet->flags & AV_PKT_FLAG_KEY));
        packet->duration = 1;
        av_packet_rescale_ts(packet, context->time_base, out->streams[0]->time_base);
        packet->stream_index = 0;
        checked(av_interleaved_write_frame(out, packet), "mux packet");
        av_packet_unref(packet);
        count++;
    }
    if (result != AVERROR(EAGAIN) && result != AVERROR_EOF) checked(result, "receive");
    return count;
}

int main(int argc, char **argv) {
    const char *name = argc > 1 ? argv[1] : "h264_nvenc";
    const char *output = argc > 2 ? argv[2] : "probe.mkv";
    const char *report = argc > 3 ? argv[3] : "packets.csv";
    const int software = !strncmp(name, "libx264", 7);
    const AVCodec *codec = avcodec_find_encoder_by_name(software ? "libx264" : name);
    if (!codec) { fprintf(stderr, "Encoder unavailable: %s\n", name); return 1; }
    AVCodecContext *context = avcodec_alloc_context3(codec);
    AVFrame *frame = av_frame_alloc();
    AVPacket *packet = av_packet_alloc();
    if (!context || !frame || !packet) return 1;
    context->width = 2560; context->height = 1440;
    context->pix_fmt = AV_PIX_FMT_NV12;
    context->time_base = (AVRational){1, 60};
    context->framerate = (AVRational){60, 1};
    context->gop_size = 120; context->max_b_frames = 0;
    context->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    context->bit_rate = context->rc_max_rate = 32000000;
    context->rc_buffer_size = 32000000;
    context->color_primaries = AVCOL_PRI_BT709;
    context->color_trc = AVCOL_TRC_BT709;
    context->colorspace = AVCOL_SPC_BT709;
    context->color_range = AVCOL_RANGE_MPEG;
    AVDictionary *options = NULL;
    if (software) {
        av_dict_set(&options, "preset", "veryfast", 0);
        av_dict_set(&options, "tune", "zerolatency", 0);
        if (!strcmp(name, "libx264")) av_dict_set(&options, "x264-params", "nal-hrd=cbr", 0);
    } else {
        av_dict_set(&options, "preset", "p4", 0);
        av_dict_set(&options, "tune", "ll", 0);
        av_dict_set(&options, "rc", "cbr", 0);
        av_dict_set(&options, "zerolatency", "1", 0);
    }
    checked(avcodec_open2(context, codec, &options), "open encoder");
    if (av_dict_count(options)) { fprintf(stderr, "Unused encoder options\n"); return 1; }
    frame->format = context->pix_fmt;
    frame->width = context->width; frame->height = context->height;
    checked(av_frame_get_buffer(frame, 32), "allocate frame");
    AVFormatContext *out = NULL;
    checked(avformat_alloc_output_context2(&out, NULL, "matroska", output), "allocate muxer");
    AVStream *stream = avformat_new_stream(out, NULL);
    if (!stream) return 1;
    stream->time_base = context->time_base;
    stream->avg_frame_rate = context->framerate;
    checked(avcodec_parameters_from_context(stream->codecpar, context), "copy codec parameters");
    checked(avio_open(&out->pb, output, AVIO_FLAG_WRITE), "open output");
    checked(avformat_write_header(out, NULL), "write header");
    FILE *csv = fopen(report, "w");
    if (!csv) return 1;
    fprintf(csv, "pts,dts,bytes,key\n");
    int count = 0;
    int64_t started = av_gettime_relative();
    for (int n = 0; n < 540; n++) {
        if (n == 180 || n == 360) {
            context->bit_rate = context->rc_max_rate = n == 180 ? 8000000 : 32000000;
            context->rc_buffer_size = (int)context->bit_rate;
            fprintf(stderr, "frame %d: request %" PRId64 " bit/s (same context)\n", n, context->bit_rate);
        }
        checked(av_frame_make_writable(frame), "writable frame");
        /* Deterministic moving textured tiles; identical input for each encoder. */
        for (int y = 0; y < frame->height; y++) {
            uint8_t *row = frame->data[0] + y * frame->linesize[0];
            for (int x = 0; x < frame->width; x++) {
                uint32_t v = (uint32_t)((x + n * 7) / 4) * 2654435761u;
                v ^= (uint32_t)((y + n * 3) / 4) * 2246822519u;
                v ^= v >> 13; v *= 3266489917u; v ^= v >> 16;
                row[x] = (uint8_t)(16 + v % 220);
            }
        }
        for (int y = 0; y < frame->height / 2; y++)
            memset(frame->data[1] + y * frame->linesize[1], 128, frame->width);
        frame->pts = n;
        checked(avcodec_send_frame(context, frame), "send frame");
        count += drain(context, packet, out, csv);
    }
    checked(avcodec_send_frame(context, NULL), "flush");
    count += drain(context, packet, out, csv);
    fprintf(stderr, "encoder=%s packets=%d elapsed_ms=%.3f\n", name, count,
            (av_gettime_relative() - started) / 1000.0);
    checked(av_write_trailer(out), "write trailer");
    checked(avio_closep(&out->pb), "close output");
    avformat_free_context(out); fclose(csv);
    av_dict_free(&options); av_packet_free(&packet);
    av_frame_free(&frame); avcodec_free_context(&context);
    return count == 540 ? 0 : 1;
}
